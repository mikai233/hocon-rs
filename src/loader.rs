mod format;
mod source;

use std::path::Path;

use crate::Result;
use crate::config_options::ConfigOptions;
use crate::error::Error;
use crate::parser::HoconParser;
use crate::parser::read::{Read, StreamRead};
use crate::raw::field::ObjectField;
use crate::raw::include::{Inclusion, Location};
use crate::raw::raw_object::RawObject;
use crate::raw::raw_value::RawValue;
use crate::syntax::Syntax;
#[cfg(feature = "urls_includes")]
use format::detect_url_syntax;
use format::{parse_json, parse_properties};
use source::{
    Candidate, Source, SourceId, discover_file_candidates, parse_non_file_url, resolve_url,
};

#[derive(Debug, Default)]
struct LoadContext {
    stack: Vec<SourceId>,
    include_depth: usize,
}

pub(crate) struct Loader {
    options: ConfigOptions,
    #[cfg(feature = "urls_includes")]
    client: reqwest::blocking::Client,
}

impl Loader {
    pub(crate) fn new(options: ConfigOptions) -> Self {
        Self {
            options,
            #[cfg(feature = "urls_includes")]
            client: reqwest::blocking::Client::new(),
        }
    }

    /// Loads a root configuration, falling back to the configured classpath and
    /// applying environment variables exactly once.
    pub(crate) fn load(&self, path: impl AsRef<Path>) -> Result<RawObject> {
        let path = path.as_ref();
        let mut context = LoadContext::default();
        let raw = match self.load_path(path, &mut context) {
            Ok(raw) => raw,
            Err(error) if is_not_found(&error) => self.load_classpath(path, &mut context)?,
            Err(error) => return Err(error),
        };

        if self.options.use_system_environment {
            Ok(RawObject::merge(parse_environment(), raw))
        } else {
            Ok(raw)
        }
    }

    /// Loads only from the supplied filesystem path. This is used by
    /// `Config::parse_file` and does not apply root-level fallback sources.
    pub(crate) fn load_file(&self, path: impl AsRef<Path>) -> Result<RawObject> {
        self.load_path(path.as_ref(), &mut LoadContext::default())
    }

    #[cfg(feature = "urls_includes")]
    pub(crate) fn load_url(&self, url: url::Url) -> Result<RawObject> {
        self.load_source(Source::Url(url), None, &mut LoadContext::default())
    }

    pub(crate) fn parse_hocon<'de, R>(&self, read: R) -> Result<RawObject>
    where
        R: Read<'de>,
    {
        let mut raw = HoconParser::with_options(read, self.options.clone()).parse()?;
        self.resolve_includes(&mut raw, None, &mut LoadContext::default())?;
        Ok(raw)
    }

    fn load_path(&self, path: &Path, context: &mut LoadContext) -> Result<RawObject> {
        let candidates = discover_file_candidates(path)?;
        self.load_candidates(candidates, context)
    }

    fn load_classpath(&self, path: &Path, context: &mut LoadContext) -> Result<RawObject> {
        if path.is_absolute() {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Absolute path in classpath",
            )));
        }

        for root in &*self.options.classpath {
            let candidate = Path::new(root).join(path);
            match self.load_path(&candidate, context) {
                Ok(raw) => return Ok(raw),
                Err(error) if is_not_found(&error) => {}
                Err(error) => return Err(error),
            }
        }

        Err(not_found(format!(
            "No configuration file was found at {} in classpath [{}]",
            path.display(),
            self.options.classpath.join(", ")
        )))
    }

    fn load_candidates(
        &self,
        mut candidates: Vec<Candidate>,
        context: &mut LoadContext,
    ) -> Result<RawObject> {
        let mut loaded = Vec::with_capacity(candidates.len());
        for candidate in candidates.drain(..) {
            let syntax = candidate.syntax;
            let raw = self.load_source(candidate.source, Some(syntax), context)?;
            loaded.push((raw, syntax));
        }

        loaded.sort_by(|left, right| (self.options.compare)(&left.1, &right.1));
        Ok(loaded
            .into_iter()
            .map(|(object, _)| object)
            .fold(RawObject::default(), RawObject::merge))
    }

    fn load_source(
        &self,
        source: Source,
        syntax: Option<Syntax>,
        context: &mut LoadContext,
    ) -> Result<RawObject> {
        let id = source.id();
        if context.stack.contains(&id) {
            return Err(Error::InclusionCycle);
        }
        context.stack.push(id);
        let result = self.read_source(&source, syntax, context);
        context.stack.pop();
        result
    }

    fn read_source(
        &self,
        source: &Source,
        syntax: Option<Syntax>,
        context: &mut LoadContext,
    ) -> Result<RawObject> {
        match source {
            Source::File(path) => {
                let file = std::fs::File::open(path)?;
                self.parse_reader(
                    std::io::BufReader::new(file),
                    syntax.unwrap(),
                    source,
                    context,
                )
            }
            Source::Url(url) => self.read_url(url, syntax, context),
        }
    }

    #[cfg(feature = "urls_includes")]
    fn read_url(
        &self,
        url: &url::Url,
        syntax: Option<Syntax>,
        context: &mut LoadContext,
    ) -> Result<RawObject> {
        let response = self.client.get(url.clone()).send()?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(not_found(url.as_str()));
        }
        let response = response.error_for_status()?;
        let detected = syntax.unwrap_or_else(|| detect_url_syntax(&response));
        let effective_source = Source::Url(response.url().clone());
        self.parse_reader(response, detected, &effective_source, context)
    }

    #[cfg(not(feature = "urls_includes"))]
    fn read_url(
        &self,
        _url: &url::Url,
        _syntax: Option<Syntax>,
        _context: &mut LoadContext,
    ) -> Result<RawObject> {
        Err(Error::UrlsIncludesDisabled)
    }

    fn parse_reader<R>(
        &self,
        reader: R,
        syntax: Syntax,
        source: &Source,
        context: &mut LoadContext,
    ) -> Result<RawObject>
    where
        R: std::io::Read,
    {
        let mut raw = match syntax {
            Syntax::Hocon => {
                let read = StreamRead::new(reader);
                HoconParser::with_options(read, self.options.clone()).parse()?
            }
            Syntax::Json => parse_json(reader)?,
            Syntax::Properties => parse_properties(reader)?,
        };
        self.resolve_includes(&mut raw, Some(source), context)?;
        Ok(raw)
    }

    fn resolve_includes(
        &self,
        object: &mut RawObject,
        origin: Option<&Source>,
        context: &mut LoadContext,
    ) -> Result<()> {
        for field in object.iter_mut() {
            match field {
                ObjectField::Inclusion { inclusion, .. } => {
                    inclusion.val = self
                        .resolve_include(inclusion, origin, context)?
                        .map(Box::new);
                }
                ObjectField::KeyValue { value, .. } => {
                    self.resolve_value_includes(value, origin, context)?;
                }
                ObjectField::NewlineComment(_) => {}
            }
        }
        Ok(())
    }

    fn resolve_value_includes(
        &self,
        value: &mut RawValue,
        origin: Option<&Source>,
        context: &mut LoadContext,
    ) -> Result<()> {
        match value {
            RawValue::Object(object) => self.resolve_includes(object, origin, context),
            RawValue::Array(array) => {
                for value in array.iter_mut() {
                    self.resolve_value_includes(value, origin, context)?;
                }
                Ok(())
            }
            RawValue::Concat(concat) => {
                for value in concat.get_values_mut() {
                    self.resolve_value_includes(value, origin, context)?;
                }
                Ok(())
            }
            RawValue::AddAssign(value) => self.resolve_value_includes(value, origin, context),
            _ => Ok(()),
        }
    }

    fn resolve_include(
        &self,
        inclusion: &Inclusion,
        origin: Option<&Source>,
        context: &mut LoadContext,
    ) -> Result<Option<RawObject>> {
        if context.include_depth >= self.options.max_include_depth {
            return Err(Error::Include {
                inclusion: inclusion.to_string(),
                error: Box::new(Error::InclusionDepthExceeded {
                    max_depth: self.options.max_include_depth,
                }),
            });
        }

        context.include_depth += 1;
        let result = self.load_include(inclusion, origin, context);
        context.include_depth -= 1;

        match result {
            Ok(object) => Ok(Some(object)),
            Err(error) if is_not_found(&error) && !inclusion.required => Ok(None),
            Err(error) => Err(Error::Include {
                inclusion: inclusion.to_string(),
                error: Box::new(error),
            }),
        }
    }

    fn load_include(
        &self,
        inclusion: &Inclusion,
        origin: Option<&Source>,
        context: &mut LoadContext,
    ) -> Result<RawObject> {
        let path = inclusion.path.as_str();
        match inclusion.location {
            Some(Location::File) => self.load_include_file(Path::new(path), origin, context),
            Some(Location::Classpath) => self.load_classpath(Path::new(path), context),
            Some(Location::Url) => {
                let url = resolve_url(path, origin)?;
                if url.scheme() == "file" {
                    let path = url
                        .to_file_path()
                        .map_err(|_| Error::InvalidFileUrl(url.to_string()))?;
                    self.load_path(&path, context)
                } else {
                    self.load_source(Source::Url(url), None, context)
                }
            }
            None => {
                if let Some(url) = parse_non_file_url(path)? {
                    return self.load_source(Source::Url(url), None, context);
                }
                if matches!(origin, Some(Source::Url(_))) {
                    let url = resolve_url(path, origin)?;
                    return self.load_source(Source::Url(url), None, context);
                }
                match self.load_include_file(Path::new(path), origin, context) {
                    Ok(raw) => Ok(raw),
                    Err(error) if is_not_found(&error) => {
                        self.load_classpath(Path::new(path), context)
                    }
                    Err(error) => Err(error),
                }
            }
        }
    }

    fn load_include_file(
        &self,
        path: &Path,
        origin: Option<&Source>,
        context: &mut LoadContext,
    ) -> Result<RawObject> {
        if let Ok(url) = url::Url::parse(path.to_string_lossy().as_ref())
            && url.scheme() == "file"
        {
            let path = url
                .to_file_path()
                .map_err(|_| Error::InvalidFileUrl(url.to_string()))?;
            return self.load_path(&path, context);
        }

        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else if let Some(Source::File(parent)) = origin {
            parent.parent().unwrap_or_else(|| Path::new("")).join(path)
        } else {
            path.to_path_buf()
        };
        self.load_path(&resolved, context)
    }
}

fn parse_environment() -> RawObject {
    let mut raw = RawObject::default();
    raw.extend(
        std::env::vars()
            .map(|(key, value)| ObjectField::key_value(key, RawValue::quoted_string(value))),
    );
    raw
}

fn is_not_found(error: &Error) -> bool {
    matches!(error, Error::Io(io) if io.kind() == std::io::ErrorKind::NotFound)
}

fn not_found(message: impl Into<String>) -> Error {
    Error::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        message.into(),
    ))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::Value;

    use super::*;

    static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let id = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("hocon-rs-loader-{}-{id}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn write(&self, path: &str, contents: &str) -> PathBuf {
            let path = self.0.join(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, contents).unwrap();
            path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn contains_error(error: &Error, predicate: impl Fn(&Error) -> bool + Copy) -> bool {
        if predicate(error) {
            return true;
        }
        match error {
            Error::Include { error, .. } => contains_error(error, predicate),
            _ => false,
        }
    }

    #[test]
    fn resolves_relative_include_from_the_including_file() {
        let dir = TestDir::new();
        let root = dir.write("config/root.conf", "include \"nested/child.conf\"");
        dir.write("config/nested/child.conf", "answer = 42");

        let raw = Loader::new(ConfigOptions::default()).load(root).unwrap();
        let value: Value = crate::Config::from(raw).resolve().unwrap();

        assert_eq!(value["answer"], Value::from(42));
    }

    #[test]
    fn detects_cycle_after_normalizing_file_paths() {
        let dir = TestDir::new();
        let root = dir.write("config/root.conf", "include \"./nested/../root.conf\"");
        std::fs::create_dir_all(dir.0.join("config/nested")).unwrap();

        let error = Loader::new(ConfigOptions::default())
            .load(root)
            .unwrap_err();

        assert!(contains_error(&error, |error| matches!(
            error,
            Error::InclusionCycle
        )));
    }

    #[test]
    fn enforces_include_depth() {
        let dir = TestDir::new();
        let root = dir.write("root.conf", "include \"one.conf\"");
        dir.write("one.conf", "include \"two.conf\"");
        dir.write("two.conf", "value = true");
        let options = ConfigOptions {
            max_include_depth: 1,
            ..ConfigOptions::default()
        };

        let error = Loader::new(options).load(root).unwrap_err();

        assert!(contains_error(&error, |error| matches!(
            error,
            Error::InclusionDepthExceeded { max_depth: 1 }
        )));
    }

    #[test]
    fn ignores_only_missing_optional_includes() {
        let dir = TestDir::new();
        let root = dir.write("root.conf", "include \"missing.conf\"\nvalue = true");

        let raw = Loader::new(ConfigOptions::default()).load(root).unwrap();
        let value: Value = crate::Config::from(raw).resolve().unwrap();

        assert_eq!(value["value"], Value::Boolean(true));
    }

    #[test]
    fn reports_missing_required_includes() {
        let dir = TestDir::new();
        let root = dir.write(
            "root.conf",
            "include required(\"missing.conf\")\nvalue = true",
        );

        let error = Loader::new(ConfigOptions::default())
            .load(root)
            .unwrap_err();

        assert!(matches!(error, Error::Include { .. }));
        assert!(contains_error(&error, is_not_found));
    }

    #[cfg(not(feature = "urls_includes"))]
    #[test]
    fn parses_url_include_before_reporting_disabled_feature() {
        let loader = Loader::new(ConfigOptions::default());
        let read = crate::parser::read::StrRead::new("include url(\"https://example.com/a.conf\")");

        let error = loader.parse_hocon(read).unwrap_err();

        assert!(contains_error(&error, |error| matches!(
            error,
            Error::UrlsIncludesDisabled
        )));
    }

    #[cfg(feature = "urls_includes")]
    #[test]
    fn resolves_relative_url_and_content_type_with_parameters() {
        use std::io::{Read as _, Write as _};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 2048];
                let length = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..length]);
                let path = request.split_whitespace().nth(1).unwrap();
                let (content_type, body) = match path {
                    "/root.conf" => ("application/hocon", "include \"child\""),
                    "/child" => ("application/json; charset=utf-8", r#"{"answer": 42}"#),
                    _ => panic!("unexpected path: {path}"),
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });

        let url = url::Url::parse(&format!("http://{address}/root.conf")).unwrap();
        let raw = Loader::new(ConfigOptions::default()).load_url(url).unwrap();
        let value: Value = crate::Config::from(raw).resolve().unwrap();
        server.join().unwrap();

        assert_eq!(value["answer"], Value::from(42));
    }
}
