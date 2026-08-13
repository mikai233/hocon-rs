use crate::Result;
use crate::error::Error;
#[cfg(feature = "properties")]
use crate::raw::field::ObjectField;
use crate::raw::raw_object::RawObject;
use crate::raw::raw_value::RawValue;

pub(super) fn parse_json<R: std::io::Read>(reader: R) -> Result<RawObject> {
    let value: serde_json::Value = serde_json::from_reader(reader)?;
    let value: RawValue = value.into();
    if let RawValue::Object(raw_object) = value {
        Ok(raw_object)
    } else {
        Err(Error::Deserialize(format!(
            "JSON must have an object as the root when parsing into HOCON, but got {}",
            value.ty()
        )))
    }
}

#[cfg(feature = "properties")]
pub(super) fn parse_properties<R: std::io::Read>(reader: R) -> Result<RawObject> {
    let properties = java_properties::read(reader)?;
    let mut raw = RawObject::default();
    raw.extend(
        properties
            .into_iter()
            .map(|(key, value)| ObjectField::key_value(key, RawValue::quoted_string(value))),
    );
    Ok(raw)
}

#[cfg(not(feature = "properties"))]
pub(super) fn parse_properties<R: std::io::Read>(_reader: R) -> Result<RawObject> {
    Err(Error::PropertiesDisabled)
}

#[cfg(feature = "urls_includes")]
pub(super) fn detect_url_syntax(response: &reqwest::blocking::Response) -> crate::syntax::Syntax {
    use std::path::Path;

    use crate::syntax::Syntax;

    let extension = response
        .url()
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .and_then(|filename| Path::new(filename).extension())
        .and_then(|extension| extension.to_str())
        .and_then(Syntax::from_extension);

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .and_then(Syntax::from_content_type);

    extension.or(content_type).unwrap_or(Syntax::Hocon)
}
