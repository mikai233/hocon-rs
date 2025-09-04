use std::path::{Path, PathBuf};

use crate::Result;
use crate::config_options::ConfigOptions;
use crate::error::Error;
use crate::parser::parser::HoconParser;
use crate::parser::read::{DEFAULT_BUFFER, StreamRead};
use crate::{
    raw::{field::ObjectField, raw_object::RawObject, raw_value::RawValue},
    syntax::Syntax,
};

#[derive(Default)]
struct ConfigPath {
    hcon: Option<PathBuf>,
    json: Option<PathBuf>,
    properties: Option<PathBuf>,
}

impl ConfigPath {
    fn set_path(&mut self, path: PathBuf, syntax: Syntax) {
        match syntax {
            Syntax::Hocon => {
                self.hcon = Some(path);
            }
            Syntax::Json => {
                self.json = Some(path);
            }
            Syntax::Properties => {
                self.properties = Some(path);
            }
        }
    }
}

fn find_config_path(path: impl AsRef<Path>) -> Result<ConfigPath> {
    let path = path.as_ref();
    let extension_syntax = if let Some(extension) = path.extension()
        && let Some(extension) = extension.to_str()
    {
        if extension == "json" {
            Some(Syntax::Json)
        } else if extension == "conf" {
            Some(Syntax::Hocon)
        } else if extension == "properties" {
            Some(Syntax::Properties)
        } else {
            None
        }
    } else {
        None
    };
    let mut config_path = ConfigPath::default();
    match extension_syntax {
        Some(syntax) => {
            let path = path.to_path_buf();
            if path.is_file() {
                config_path.set_path(path, syntax);
            } else {
                return Err(Error::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    path.display().to_string(),
                )));
            }
        }
        None => {
            let mut json_path = path.to_path_buf();
            json_path.set_extension("json");
            let mut hocon_path = path.to_path_buf();
            hocon_path.set_extension("conf");
            let mut properties_path = path.to_path_buf();
            properties_path.set_extension("properties");
            if json_path.is_file() {
                config_path.set_path(json_path, Syntax::Json);
            }
            if hocon_path.is_file() {
                config_path.set_path(hocon_path, Syntax::Hocon);
            }
            if properties_path.is_file() {
                config_path.set_path(properties_path, Syntax::Properties);
            }
        }
    }
    if [
        &config_path.hcon,
        &config_path.json,
        &config_path.properties,
    ]
    .iter()
    .all(|p| p.is_none())
    {
        let message = format!(
            "No configuration file (.conf, .json, .properties) was found at the given path: {}",
            path.display(),
        );
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            message,
        )));
    }
    Ok(config_path)
}

pub(crate) fn load_from_path(path: impl AsRef<Path>, options: ConfigOptions) -> Result<RawObject> {
    let config_path = find_config_path(&path)?;
    let mut result = vec![];
    if let Some(hocon) = config_path.hcon {
        let file = std::fs::File::open(hocon)?;
        let reader = std::io::BufReader::new(file);
        let read: StreamRead<_, DEFAULT_BUFFER> = StreamRead::new(reader);
        let raw_obj = parse_hocon(read, options.clone())?;
        result.push((raw_obj, Syntax::Hocon));
    }
    if let Some(json) = config_path.json {
        let file = std::fs::File::open(json)?;
        let reader = std::io::BufReader::new(file);
        let raw_obj = parse_json(reader)?;
        result.push((raw_obj, Syntax::Json));
    }
    if let Some(properties) = config_path.properties {
        let file = std::fs::File::open(properties)?;
        let reader = std::io::BufReader::new(file);
        let raw_obj = parse_properties(reader)?;
        result.push((raw_obj, Syntax::Json));
    }
    let cmp = &options.compare;
    result.sort_by(|a, b| cmp(&a.1, &b.1));
    let raw = result
        .into_iter()
        .map(|(o, _)| o)
        .fold(RawObject::default(), |merged, o| {
            RawObject::merge(merged, o)
        });
    Ok(raw)
}

#[cfg(feature = "urls_includes")]
pub(crate) fn load_from_url(url: url::Url, options: ConfigOptions) -> Result<RawObject> {
    let client = reqwest::blocking::Client::new();
    match client.get(url).send() {
        Ok(response) => {
            let extension_syntax = if let Some(filename) = response
                .url()
                .path_segments()
                .and_then(|segments| segments.last())
            {
                if let Some(dot_index) = filename.rfind('.') {
                    let extension = &filename[dot_index + 1..];
                    match extension {
                        "json" => Some(Syntax::Json),
                        "properties" => Some(Syntax::Properties),
                        "conf" => Some(Syntax::Hocon),
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            };
            let header_syntax =
                if let Some(content_type) = response.headers().get(reqwest::header::CONTENT_TYPE) {
                    match content_type.as_bytes() {
                        b"application/json" => Some(Syntax::Json),
                        b"text/x-java-properties" => Some(Syntax::Properties),
                        b"application/hocon" => Some(Syntax::Hocon),
                        _ => None,
                    }
                } else {
                    None
                };
            let syntax = extension_syntax.or(header_syntax).unwrap_or(Syntax::Hocon);
            match syntax {
                Syntax::Hocon => {
                    let read: StreamRead<_, 4096> =
                        StreamRead::new(std::io::BufReader::new(response));
                    parse_hocon(read, options)
                }
                Syntax::Json => parse_json(response),
                Syntax::Properties => parse_properties(response),
            }
        }
        Err(error) => Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            error,
        ))),
    }
}

pub(crate) fn load_from_classpath(
    path: impl AsRef<Path>,
    options: ConfigOptions,
) -> Result<RawObject> {
    let path = path.as_ref();
    if path.is_absolute() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Absolute path in classpath",
        )));
    }
    for classpath in &*options.classpath {
        let candidate = Path::new(classpath).join(path);
        match load_from_path(&candidate, options.clone()) {
            Ok(raw) => {
                return Ok(raw);
            }
            Err(crate::error::Error::Io(_)) => {}
            error => {
                return error;
            }
        }
    }
    let message = format!(
        "No configuration file (.conf, .json, .properties) was found at the given path: {} in classpath: [{}]",
        path.display(),
        options.classpath.join(", ")
    );
    return Err(Error::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        message,
    )));
}

fn parse_json<R>(reader: R) -> Result<RawObject>
where
    R: std::io::Read,
{
    let value: serde_json::Value = serde_json::from_reader(reader)?;
    let value: RawValue = value.into();
    if let RawValue::Object(raw_object) = value {
        Ok(raw_object)
    } else {
        Err(crate::error::Error::DeserializeError(format!(
            "JSON must have an object as the root when parsing into HOCON, but got {}",
            value.ty()
        )))
    }
}

pub(crate) fn parse_hocon<R>(read: R, options: ConfigOptions) -> Result<RawObject>
where
    R: crate::parser::read::Read,
{
    HoconParser::with_options(read, options).parse()
}

fn parse_properties<R>(reader: R) -> crate::Result<RawObject>
where
    R: std::io::Read,
{
    let properties = java_properties::read(reader)?;
    let mut raw_object = RawObject::default();
    let properties = properties
        .into_iter()
        .map(|(key, value)| ObjectField::key_value(key, RawValue::quoted_string(value)));
    raw_object.extend(properties);
    Ok(raw_object)
}

fn parse_environments() -> RawObject {
    let mut raw = RawObject::default();
    for (key, value) in std::env::vars() {
        raw.push(ObjectField::key_value(key, RawValue::quoted_string(value)));
    }
    raw
}

pub(crate) fn load(path: impl AsRef<Path>, options: ConfigOptions) -> Result<RawObject> {
    let env_raw = if options.use_system_environment {
        Some(parse_environments())
    } else {
        None
    };
    let path = path.as_ref();
    let raw = match load_from_path(path, options.clone()) {
        Ok(raw) => raw,
        Err(Error::Io(io)) if io.kind() == std::io::ErrorKind::NotFound => {
            load_from_classpath(path, options)?
        }
        error => {
            return error;
        }
    };
    let raw_obj = match env_raw {
        Some(env_raw) => RawObject::merge(env_raw, raw),
        None => raw,
    };
    Ok(raw_obj)
}
