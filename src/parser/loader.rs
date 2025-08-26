use std::path::{Path, PathBuf};

use crate::parser::error::HoconParseError;
use crate::{
    parser::{config_parse_options::ConfigParseOptions, parse},
    raw::{field::ObjectField, raw_object::RawObject, raw_value::RawValue},
    syntax::Syntax,
};
use itertools::Itertools;
use nom::Needed;
use nom_language::error::convert_error;

struct ConfigPath {
    path: PathBuf,
    syntax: Syntax,
}

fn load_config_paths(path: impl AsRef<Path>) -> Vec<ConfigPath> {
    let path = path.as_ref();
    let syntax = if let Some(extension) = path.extension()
        && let Some(extension) = extension.to_str()
    {
        if extension.eq_ignore_ascii_case("json") {
            Some(Syntax::Json)
        } else if extension.eq_ignore_ascii_case("conf") {
            Some(Syntax::Hocon)
        } else if extension.eq_ignore_ascii_case("properties") {
            Some(Syntax::Properties)
        } else {
            None
        }
    } else {
        None
    };
    let mut result = vec![];
    match syntax {
        Some(syntax) => {
            let mut path = path.to_path_buf();
            path.set_extension(syntax.to_string());
            if path.is_file() {
                let config_path = ConfigPath { path, syntax };
                result.push(config_path);
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
                let config_path = ConfigPath {
                    path: json_path,
                    syntax: Syntax::Json,
                };
                result.push(config_path);
            }
            if hocon_path.is_file() {
                let config_path = ConfigPath {
                    path: hocon_path,
                    syntax: Syntax::Hocon,
                };
                result.push(config_path);
            }
            if properties_path.is_file() {
                let config_path = ConfigPath {
                    path: properties_path,
                    syntax: Syntax::Properties,
                };
                result.push(config_path);
            }
        }
    }
    result
}

pub(crate) fn load_from_path(
    path: impl AsRef<Path>,
    parse_opts: ConfigParseOptions,
) -> crate::Result<RawObject> {
    let path_ref = path.as_ref();
    let paths = load_config_paths(path_ref);
    fn construct_error(path: &Path, io: Option<std::io::Error>) -> crate::error::Error {
        let message = format!(
            "No configuration file (.conf, .json, .properties) was found at the given path: {}",
            path.display()
        );
        crate::error::Error::ConfigNotFound {
            message,
            error: io.map(|e| Box::new(e) as Box<dyn std::error::Error>),
        }
    }
    if paths.is_empty() {
        let message = format!(
            "No configuration file (.conf, .json, .properties) was found at the given path: {}",
            path_ref.display()
        );
        let error = crate::error::Error::ConfigNotFound {
            message,
            error: None,
        };
        return Err(error);
    }
    let mut result = vec![];
    for ConfigPath { path, syntax } in paths {
        let raw = match syntax {
            Syntax::Hocon => match std::fs::read_to_string(&path) {
                Ok(data) => load_hocon(&data, parse_opts.clone())?,
                Err(error) => {
                    return Err(construct_error(&path, Some(error)));
                }
            },
            Syntax::Json => match std::fs::File::open(&path) {
                Ok(file) => {
                    let reader = std::io::BufReader::new(file);
                    load_json(reader)?
                }
                Err(error) => {
                    return Err(construct_error(&path, Some(error)));
                }
            },
            Syntax::Properties => match std::fs::File::open(&path) {
                Ok(file) => {
                    let reader = std::io::BufReader::new(file);
                    load_properties(reader)?
                }
                Err(error) => {
                    return Err(construct_error(&path, Some(error)));
                }
            },
        };
        result.push((raw, syntax));
    }
    let cmp = parse_opts.options.compare;
    let raw = result
        .into_iter()
        .sorted_by(|(_, s1), (_, s2)| cmp(s1, s2))
        .map(|(o, _)| o)
        .fold(RawObject::default(), |merged, o| {
            RawObject::merge(merged, o)
        });
    Ok(raw)
}

pub(crate) fn load_from_url(
    url: url::Url,
    parse_opts: ConfigParseOptions,
) -> crate::Result<RawObject> {
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
                Syntax::Hocon => match response.text() {
                    Ok(data) => load_hocon(&data, parse_opts),
                    Err(error) => {
                        let message = format!(
                            "Can't get response content from {}, error: {}",
                            error.url().unwrap().as_str(),
                            error
                        );
                        let error = Box::new(error);
                        Err(crate::error::Error::ConfigNotFound {
                            message,
                            error: Some(error),
                        })
                    }
                },
                Syntax::Json => load_json(response),
                Syntax::Properties => load_properties(response),
            }
        }
        Err(error) => {
            let message = format!(
                "Url resource not found: {}, error: {}",
                error.url().unwrap().as_str(),
                error
            );
            let error = Box::new(error);
            Err(crate::error::Error::ConfigNotFound {
                message,
                error: Some(error),
            })
        }
    }
}

pub(crate) fn load_from_classpath(
    path: impl AsRef<Path>,
    parse_opts: ConfigParseOptions,
) -> crate::Result<RawObject> {
    let path = path.as_ref();
    if path.is_absolute() {
        return Err(crate::error::Error::AbsolutePathInClasspath(
            path.display().to_string(),
        ));
    }
    if !parse_opts.options.classpath.is_empty() {
        for classpath in &parse_opts.options.classpath {
            let candidate = Path::new(classpath).join(path);
            match load_from_path(&candidate, parse_opts.clone()) {
                Ok(raw) => {
                    return Ok(raw);
                }
                Err(crate::error::Error::ConfigNotFound { .. }) => {}
                error => {
                    return error;
                }
            }
        }
    }
    let message = format!(
        "No configuration file (.conf, .json, .properties) was found at the given path: {} in classpath: [{}]",
        path.display(),
        parse_opts.options.classpath.join(", ")
    );
    return Err(crate::error::Error::ConfigNotFound {
        message,
        error: None,
    });
}

fn load_json<R>(reader: R) -> crate::Result<RawObject>
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

fn load_hocon(s: &str, options: ConfigParseOptions) -> crate::Result<RawObject> {
    let raw_object = match parse(s, options) {
        Ok((_, raw)) => raw,
        Err(error) => {
            return match error {
                nom::Err::Incomplete(i) => {
                    let size = match i {
                        Needed::Unknown => "Unknown".to_string(),
                        Needed::Size(s) => s.get().to_string(),
                    };
                    Err(crate::error::Error::ParseError(format!(
                        "Incomplete parse: {}",
                        size
                    )))
                }
                nom::Err::Error(e) | nom::Err::Failure(e) => match e {
                    HoconParseError::Nom(e) => {
                        let err_msg = convert_error(s, e);
                        Err(crate::error::Error::ParseError(err_msg))
                    }
                    HoconParseError::Other(error) => Err(error),
                },
            };
        }
    };
    Ok(raw_object)
}

fn load_properties<R>(reader: R) -> crate::Result<RawObject>
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

fn load_environments() -> RawObject {
    let mut raw = RawObject::default();
    for (key, value) in std::env::vars() {
        raw.push(ObjectField::key_value(key, RawValue::quoted_string(value)));
    }
    raw
}

pub(crate) fn load(
    path: impl AsRef<Path>,
    parse_opts: ConfigParseOptions,
) -> crate::Result<RawObject> {
    let env_raw = if parse_opts.options.use_system_environment {
        Some(load_environments())
    } else {
        None
    };
    let path = path.as_ref();
    let raw = match load_from_path(path, parse_opts.clone()) {
        Ok(raw) => raw,
        Err(crate::error::Error::ConfigNotFound { .. }) => load_from_classpath(path, parse_opts.into())?,
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
