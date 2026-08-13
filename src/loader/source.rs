use std::path::{Path, PathBuf};

use crate::Result;
use crate::error::Error;
use crate::syntax::Syntax;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Source {
    File(PathBuf),
    Url(url::Url),
}

impl Source {
    pub(super) fn id(&self) -> SourceId {
        match self {
            Source::File(path) => SourceId::File(path.clone()),
            Source::Url(url) => SourceId::Url(url.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SourceId {
    File(PathBuf),
    Url(url::Url),
}

#[derive(Debug)]
pub(super) struct Candidate {
    pub(super) source: Source,
    pub(super) syntax: Syntax,
}

pub(super) fn discover_file_candidates(path: &Path) -> Result<Vec<Candidate>> {
    if let Some(syntax) = path
        .extension()
        .and_then(|value| value.to_str())
        .and_then(Syntax::from_extension)
    {
        return file_candidate(path, syntax).map(|candidate| vec![candidate]);
    }

    let mut candidates = Vec::new();
    for syntax in Syntax::enabled() {
        let candidate_path = path.with_extension(syntax.extension());
        if candidate_path.is_file() {
            candidates.push(candidate_for_existing_file(candidate_path, syntax)?);
        }
    }

    if candidates.is_empty() {
        Err(not_found(format!(
            "No enabled configuration file was found at {}",
            path.display()
        )))
    } else {
        Ok(candidates)
    }
}

pub(super) fn parse_non_file_url(value: &str) -> Result<Option<url::Url>> {
    match url::Url::parse(value) {
        Ok(url) if url.scheme() != "file" => Ok(Some(url)),
        Ok(_) | Err(url::ParseError::RelativeUrlWithoutBase) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn resolve_url(value: &str, origin: Option<&Source>) -> Result<url::Url> {
    if let Ok(url) = url::Url::parse(value) {
        return Ok(url);
    }
    if let Some(Source::Url(base)) = origin {
        return Ok(base.join(value)?);
    }
    Ok(url::Url::parse(value)?)
}

fn file_candidate(path: &Path, syntax: Syntax) -> Result<Candidate> {
    if !path.is_file() {
        return Err(not_found(path.display().to_string()));
    }
    candidate_for_existing_file(path.to_path_buf(), syntax)
}

fn candidate_for_existing_file(path: PathBuf, syntax: Syntax) -> Result<Candidate> {
    Ok(Candidate {
        source: Source::File(std::fs::canonicalize(path)?),
        syntax,
    })
}

fn not_found(message: impl Into<String>) -> Error {
    Error::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        message.into(),
    ))
}
