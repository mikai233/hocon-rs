use crate::Result;
use crate::config_options::ConfigOptions;
use crate::error::Error;
use crate::parser::loader::{self, load_from_classpath, load_from_path};
use crate::parser::read::Read;
use crate::parser::{Context, HoconParser};
use crate::raw::include::{Inclusion, Location};
use crate::raw::raw_object::RawObject;
use std::str::FromStr;

pub(crate) const INCLUDE: &[u8] = b"include";

impl<'de, R: Read<'de>> HoconParser<R> {
    pub(crate) fn parse_include(&mut self) -> Result<Inclusion> {
        self.parse_include_token()?;
        Self::drop_horizontal_whitespace(&mut self.reader)?;
        let required = self.parse_required_token()?;
        let location = self.parse_location_token()?;
        self.scratch.clear();
        let include_path = Self::parse_quoted_string(&mut self.reader, &mut self.scratch, true)?;
        for _ in [location.is_some(), required].iter().filter(|x| **x) {
            Self::drop_horizontal_whitespace(&mut self.reader)?;
            let ch = self.reader.peek()?;
            if ch != b')' {
                return Err(self.reader.peek_error(")"));
            } else {
                self.reader.discard(1)?;
            }
        }
        let inclusion = Inclusion::new(include_path.into(), required, location, None);
        Ok(inclusion)
    }

    fn parse_include_token(&mut self) -> Result<()> {
        let bytes = self.reader.peek_n(INCLUDE.len())?;
        if bytes != INCLUDE {
            return Err(self.reader.peek_error("include"));
        }
        self.reader.discard(INCLUDE.len())?;
        Ok(())
    }

    fn parse_required_token(&mut self) -> Result<bool> {
        let mut required = false;
        let ch = self.reader.peek()?;
        const REQUIRED: &[u8] = b"required(";
        if ch == b'r' {
            match self.reader.peek_n(REQUIRED.len()) {
                Ok(bytes) if bytes == REQUIRED => (),
                _ => {
                    return Err(self.reader.peek_error("required("));
                }
            }
            self.reader.discard(REQUIRED.len())?;
            required = true
        }
        if required {
            Self::drop_horizontal_whitespace(&mut self.reader)?;
        }
        Ok(required)
    }

    fn parse_location_token(&mut self) -> Result<Option<Location>> {
        let ch = self.reader.peek()?;
        const FILE: &[u8] = b"file(";
        let location = match ch {
            b'f' => {
                match self.reader.peek_n(FILE.len()) {
                    Ok(bytes) if bytes == FILE => (),
                    _ => {
                        return Err(self.reader.peek_error("file("));
                    }
                }
                self.reader.discard(FILE.len())?;
                Some(Location::File)
            }
            #[cfg(feature = "urls_includes")]
            b'u' => {
                const URL: &[u8] = b"url(";
                match self.reader.peek_n(URL.len()) {
                    Ok(bytes) if bytes == URL => (),
                    _ => {
                        return Err(self.reader.peek_error("url("));
                    }
                }
                self.reader.discard(URL.len())?;
                Some(Location::Url)
            }
            #[cfg(not(feature = "urls_includes"))]
            b'u' => {
                return Err(Error::UrlsIncludesDisabled);
            }
            b'c' => {
                const CLASSPATH: &[u8] = b"classpath(";
                match self.reader.peek_n(CLASSPATH.len()) {
                    Ok(bytes) if bytes == CLASSPATH => (),
                    _ => {
                        return Err(self.reader.peek_error("classpath("));
                    }
                }
                self.reader.discard(CLASSPATH.len())?;
                Some(Location::Classpath)
            }
            b'"' => None,
            _ => {
                return Err(self.reader.peek_error("file( or classpath( or url( or \""));
            }
        };
        if location.is_some() {
            Self::drop_horizontal_whitespace(&mut self.reader)?;
        }
        Ok(location)
    }

    fn handle_include_error<'a, F>(
        load: F,
        options: ConfigOptions,
        inclusion: &'a mut Inclusion,
        ctx: Option<Context>,
    ) -> Result<()>
    where
        F: FnOnce(&'a std::path::Path, ConfigOptions, Option<Context>) -> Result<RawObject>,
    {
        match load((**inclusion.path).as_ref(), options, ctx) {
            Ok(object) => {
                inclusion.val = Some(object.into());
            }
            Err(Error::Io(io)) if io.kind() == std::io::ErrorKind::NotFound => {
                if inclusion.required {
                    return Err(Error::Include {
                        inclusion: inclusion.to_string(),
                        error: Box::new(Error::Io(io)),
                    });
                }
            }
            Err(e) => {
                return Err(Error::Include {
                    inclusion: inclusion.to_string(),
                    error: Box::new(e),
                });
            }
        }
        Ok(())
    }

    fn inclusion_from_file(&self, inclusion: &mut Inclusion, ctx: Option<Context>) -> Result<()> {
        Self::handle_include_error(load_from_path, self.options.clone(), inclusion, ctx)
    }

    fn inclusion_from_classpath(
        &self,
        inclusion: &mut Inclusion,
        ctx: Option<Context>,
    ) -> Result<()> {
        Self::handle_include_error(load_from_classpath, self.options.clone(), inclusion, ctx)
    }

    fn inclusion_from_file_and_classpath(
        &self,
        inclusion: &mut Inclusion,
        ctx: Option<Context>,
    ) -> Result<()> {
        Self::handle_include_error(loader::load, self.options.clone(), inclusion, ctx)
    }

    #[cfg(feature = "urls_includes")]
    fn inclusion_from_url(&self, inclusion: &mut Inclusion, ctx: Option<Context>) -> Result<()> {
        let url = url::Url::from_str(&inclusion.path)?;
        match loader::load_from_url(url, self.options.clone(), ctx) {
            Ok(object) => {
                inclusion.val = Some(object.into());
            }
            Err(Error::Io(io)) if io.kind() == std::io::ErrorKind::NotFound => {
                if inclusion.required {
                    return Err(Error::Include {
                        inclusion: inclusion.to_string(),
                        error: Box::new(Error::Io(io)),
                    });
                }
            }
            Err(e) => {
                return Err(Error::Include {
                    inclusion: inclusion.to_string(),
                    error: Box::new(e),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn parse_inclusion(&self, inclusion: &mut Inclusion) -> Result<()> {
        let has_cycle = self
            .ctx
            .include_chain
            .iter()
            .rfind(|p| **p == inclusion.path)
            .is_some();
        if has_cycle {
            return Err(Error::InclusionCycle);
        }
        let mut ctx = self.ctx.clone();
        ctx.include_chain.push(inclusion.path.clone());
        match inclusion.location {
            #[cfg(feature = "urls_includes")]
            None | Some(Location::Url) => match url::Url::from_str(&inclusion.path) {
                Ok(url) => {
                    if url.scheme() != "file" {
                        self.inclusion_from_url(inclusion, Some(ctx))?;
                    }
                }
                _ => {
                    self.inclusion_from_file_and_classpath(inclusion, Some(ctx))?;
                }
            },
            #[cfg(not(feature = "urls_includes"))]
            None => match url::Url::from_str(&inclusion.path) {
                Ok(url) if url.scheme() != "file" => {
                    return Err(Error::UrlsIncludesDisabled);
                }
                _ => self.inclusion_from_file_and_classpath(inclusion, Some(ctx))?,
            },
            Some(Location::Classpath) => self.inclusion_from_classpath(inclusion, Some(ctx))?,
            Some(Location::File) => self.inclusion_from_file(inclusion, Some(ctx))?,
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Result;
    use crate::parser::HoconParser;
    use crate::parser::read::StrRead;
    use rstest::rstest;

    #[rstest]
    #[case("include \"demo\".conf", "include \"demo\"", ".conf")]
    #[case("include\"demo.conf\"", "include \"demo.conf\"", "")]
    #[case(
        "include   required(    \"demo.conf\" )",
        "include required(\"demo.conf\")",
        ""
    )]
    #[case(
        "include   required(  file(  \"demo.conf\" ))",
        "include required(file(\"demo.conf\"))",
        ""
    )]
    fn test_valid_include(
        #[case] input: &str,
        #[case] expected: &str,
        #[case] rest: &str,
    ) -> Result<()> {
        let read = StrRead::new(input);
        let mut parser = HoconParser::new(read);
        let inclusion = parser.parse_include()?;
        assert_eq!(inclusion.to_string(), expected);
        assert_eq!(parser.reader.rest()?, rest);
        Ok(())
    }

    #[rstest]
    #[case("includedemo")]
    #[case("include required (\"demo\")")]
    #[case("include required(\"demo\",)")]
    #[case("include required(\"demo\"")]
    #[case("include required1(\"demo\")")]
    #[case("include classpat(\"demo\")")]
    #[case("include classpath(file(\"demo\"))")]
    #[case("include classpath(required(\"demo\"))")]
    fn test_invalid_include(#[case] input: &str) -> Result<()> {
        let read = StrRead::new(input);
        let mut parser = HoconParser::new(read);
        let result = parser.parse_include();
        assert!(result.is_err());
        Ok(())
    }
}
