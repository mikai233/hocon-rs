use crate::Result;
use crate::config_options::ConfigOptions;
use crate::error::Error;
use crate::parser::loader::{self, load_from_classpath, load_from_path};
use crate::parser::read::Read;
use crate::parser::{Context, HoconParser};
use crate::raw::include::{Inclusion, Location};
use crate::raw::raw_object::RawObject;
use std::str::FromStr;

pub(crate) const INCLUDE: [u8; 7] = [b'i', b'n', b'c', b'l', b'u', b'd', b'e'];

impl<'de, R: Read<'de>> HoconParser<R> {
    pub(crate) fn parse_include(&mut self) -> Result<Inclusion> {
        self.parse_include_token()?;
        self.drop_horizontal_whitespace()?;
        let required = self.parse_required_token()?;
        let location = self.parse_location_token()?;
        let include_path = self.parse_quoted_string()?;
        for _ in [location.is_some(), required].iter().filter(|x| **x) {
            self.drop_horizontal_whitespace()?;
            let ch = self.reader.peek()?;
            if ch != b')' {
                return Err(Error::UnexpectedToken {
                    expected: ")",
                    found_beginning: ch,
                });
            } else {
                self.reader.next()?;
            }
        }
        let inclusion = Inclusion::new(include_path.into(), required, location, None);
        Ok(inclusion)
    }

    fn parse_include_token(&mut self) -> Result<()> {
        let ch = self.reader.peek()?;
        if ch != b'i' {
            return Err(Error::UnexpectedToken {
                expected: "include",
                found_beginning: ch,
            });
        }
        // At this point, we still don't know if it's an include or something else,
        // so we need to use peek instead of consuming it
        const N: usize = 7;
        let bytes = self.reader.peek_n::<N>()?;
        if bytes != INCLUDE {
            let (_, ch) = bytes
                .iter()
                .enumerate()
                .find(|(index, ch)| &&INCLUDE[*index] != ch)
                .unwrap();
            return Err(Error::UnexpectedToken {
                expected: "include",
                found_beginning: *ch,
            });
        }
        for _ in 0..N {
            self.reader.next()?;
        }
        Ok(())
    }

    fn parse_required_token(&mut self) -> Result<bool> {
        let mut required = false;
        let ch = self.reader.peek()?;
        const REQUIRED: [u8; 9] = [b'r', b'e', b'q', b'u', b'i', b'r', b'e', b'd', b'('];
        if ch == b'r' {
            for ele in REQUIRED {
                let next = self.reader.next()?;
                if ele != next {
                    return Err(Error::UnexpectedToken {
                        expected: "required(",
                        found_beginning: next,
                    });
                }
            }
            required = true
        }
        if required {
            self.drop_horizontal_whitespace()?;
        }
        Ok(required)
    }

    fn parse_location_token(&mut self) -> Result<Option<Location>> {
        let ch = self.reader.peek()?;
        const FILE: [u8; 5] = [b'f', b'i', b'l', b'e', b'('];
        let location = match ch {
            b'f' => {
                for ele in FILE {
                    let next = self.reader.next()?;
                    if ele != next {
                        return Err(Error::UnexpectedToken {
                            expected: "file(",
                            found_beginning: next,
                        });
                    }
                }
                Some(Location::File)
            }
            #[cfg(feature = "urls_includes")]
            b'u' => {
                const URL: [u8; 4] = [b'u', b'r', b'l', b'('];
                for ele in URL {
                    let next = self.reader.next()?;
                    if ele != next {
                        return Err(Error::UnexpectedToken {
                            expected: "url(",
                            found_beginning: next,
                        });
                    }
                }
                Some(Location::Url)
            }
            #[cfg(not(feature = "urls_includes"))]
            b'u' => {
                return Err(Error::UrlsIncludesDisabled);
            }
            b'c' => {
                const CLASSPATH: [u8; 10] =
                    [b'c', b'l', b'a', b's', b's', b'p', b'a', b't', b'h', b'('];
                for ele in CLASSPATH {
                    let next = self.reader.next()?;
                    if ele != next {
                        return Err(Error::UnexpectedToken {
                            expected: "classpath(",
                            found_beginning: next,
                        });
                    }
                }
                Some(Location::Classpath)
            }
            b'"' => None,
            ch => {
                return Err(Error::UnexpectedToken {
                    expected: "file( or classpath( or url( or \"",
                    found_beginning: ch,
                });
            }
        };
        if location.is_some() {
            self.drop_horizontal_whitespace()?;
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
    use crate::parser::read::TestRead;
    use rstest::rstest;

    #[rstest]
    #[case(vec!["i","nclude"," ", "\"demo\".conf"],"include \"demo\"", ".conf")]
    #[case(vec!["i","nclude", "\"demo.conf\""],"include \"demo.conf\"", "")]
    #[case(vec!["i","nclude","   r","equired(  ", "  \"demo.conf\" ",")"],"include required(\"demo.conf\")","" )]
    #[case(vec!["i","nclude","   r","equired(  ", "file(  \"demo.conf\" )",")"],"include required(file(\"demo.conf\"))","")]
    fn test_valid_include(
        #[case] input: Vec<&str>,
        #[case] expected: &str,
        #[case] rest: &str,
    ) -> Result<()> {
        let read = TestRead::from_input(input);
        let mut parser = HoconParser::new(read);
        let inclusion = parser.parse_include()?;
        assert_eq!(inclusion.to_string(), expected);
        assert_eq!(parser.reader.rest(), rest);
        Ok(())
    }

    #[rstest]
    #[case(vec!["include", "demo"])]
    #[case(vec!["include", "required (\"demo\")"])]
    #[case(vec!["include", "required(\"demo\",)"])]
    #[case(vec!["include", "required(\"demo\""])]
    #[case(vec!["include", "required1(\"demo\")"])]
    #[case(vec!["include", "classpat(\"demo\")"])]
    #[case(vec!["include", "classpath(file(\"demo\"))"])]
    #[case(vec!["include", "classpath(required(\"demo\"))"])]
    fn test_invalid_include(#[case] input: Vec<&str>) -> Result<()> {
        let read = TestRead::from_input(input);
        let mut parser = HoconParser::new(read);
        let result = parser.parse_include();
        assert!(result.is_err());
        Ok(())
    }
}
