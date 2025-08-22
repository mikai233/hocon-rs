use std::path::Path;

use itertools::Itertools;

use crate::{
    parser::{config_parse_options::ConfigParseOptions, parse},
    raw::{field::ObjectField, raw_object::RawObject, raw_value::RawValue},
    syntax::Syntax,
};

// If no syntax is specified and the file has no clear extension,
// this function will attempt to load all supported config formats.
pub(crate) fn load_from_file(
    path: impl AsRef<Path>,
    options: Option<ConfigParseOptions>,
    mut syntax: Option<Syntax>,
) -> crate::Result<RawObject> {
    let path = path.as_ref();
    let extension_syntax = if let Some(extension) = path.extension()
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
    syntax = syntax.or(extension_syntax);
    match syntax {
        Some(syntax) => match syntax {
            Syntax::Hocon => {
                let config = std::fs::read_to_string(path)?;
                load_hocon(&config, options)
            }
            Syntax::Json => {
                let file = std::fs::File::open(path)?;
                let reader = std::io::BufReader::new(file);
                load_json(reader)
            }
            Syntax::Properties => todo!(),
        },
        None => {
            let hocon = match std::fs::read_to_string(path.join(".conf")) {
                Ok(config) => {
                    let hocon = load_hocon(&config, options)?;
                    Some(hocon)
                }
                Err(_) => None,
            };
            let json = match std::fs::File::open(path.join(".json")) {
                Ok(file) => {
                    let reader = std::io::BufReader::new(file);
                    let json = load_json(reader)?;
                    Some(json)
                }
                Err(_) => None,
            };
            let properties = match std::fs::File::open(path.join(".properties")) {
                Ok(file) => {
                    let reader = std::io::BufReader::new(file);
                    let properties = load_properties(reader)?;
                    Some(properties)
                }
                Err(_) => None,
            };
            let raw_object = [
                (hocon, Syntax::Hocon),
                (json, Syntax::Json),
                (properties, Syntax::Properties),
            ]
            .into_iter()
            .sorted_by(|(_, s1), (_, s2)| s1.cmp(s2))
            .map(|(o, _)| o)
            .flatten()
            .fold(Some(RawObject::default()), |merged, o| match merged {
                Some(merged) => Some(RawObject::merge(merged, o)),
                None => Some(o),
            });
            let raw_object = raw_object.ok_or(crate::error::Error::ConfigNotFound(format!(
                "No configuration file (.conf, .json, .properties) was found at the given path: {}",
                path.display()
            )))?;
            Ok(raw_object)
        }
    }
}

pub(crate) fn load_from_url(
    url: reqwest::Url,
    options: Option<ConfigParseOptions>,
    syntax: Option<Syntax>,
) -> crate::Result<RawObject> {
    if url.scheme() == "file" {
        let path = url.to_file_path().unwrap();
        load_from_file(path, options, syntax)
    } else {
        let client = reqwest::blocking::Client::new();
        let response = client.get(url).send()?;
        let syntax =
            if let Some(content_type) = response.headers().get(reqwest::header::CONTENT_TYPE) {
                match content_type.as_bytes() {
                    b"application/json" => Syntax::Json,
                    b"text/x-java-properties" => Syntax::Properties,
                    b"application/hocon" => Syntax::Hocon,
                    _ => Syntax::Hocon,
                }
            } else {
                Syntax::Hocon
            };
        match syntax {
            Syntax::Hocon => {
                let config = response.text()?;
                load_hocon(&config, options)
            }
            Syntax::Json => load_json(response),
            Syntax::Properties => load_properties(response),
        }
    }
}

pub(crate) fn load_from_classpath() -> crate::Result<RawObject> {
    unimplemented!()
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

fn load_hocon(s: &str, options: Option<ConfigParseOptions>) -> crate::Result<RawObject> {
    let (_, raw_object) = parse(s, options).unwrap();
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
