use std::str::FromStr;

use crate::parser::config_parse_options::ConfigParseOptions;
use crate::parser::loader::{self, load_from_classpath, load_from_path, load_from_url};
use crate::parser::string::parse_quoted_string;
use crate::parser::{CONFIG, R, hocon_horizontal_space0, hocon_multi_space0};
use crate::raw::include::{Inclusion, Location};
use nom::Parser;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::char;
use nom::combinator::value;
use nom::sequence::{delimited, preceded};

fn parse_with_location(input: &str) -> R<'_, Inclusion> {
    let (remainder, (location, path)) = (
        alt((
            value(Location::File, tag("file")),
            // #[cfg(feature = "url")]
            value(Location::Url, tag("url")),
            value(Location::Classpath, tag("classpath")),
        )),
        delimited(
            char('('),
            delimited(
                hocon_horizontal_space0,
                parse_quoted_string,
                hocon_horizontal_space0,
            ),
            char(')'),
        ),
    )
        .parse_complete(input)?;
    let inclusion = Inclusion::new(path, false, Some(location), None);
    Ok((remainder, inclusion))
}

fn parse_with_required(input: &str) -> R<'_, Inclusion> {
    delimited(
        tag("required("),
        delimited(
            hocon_horizontal_space0,
            alt((
                parse_with_location.map(|mut inclusion| {
                    inclusion.required = true;
                    inclusion
                }),
                parse_quoted_string.map(|path| Inclusion::new(path, true, None, None)),
            )),
            hocon_horizontal_space0,
        ),
        tag(")"),
    )
    .parse_complete(input)
}

fn parse(input: &str) -> R<'_, Inclusion> {
    preceded(
        (hocon_multi_space0, tag("include")),
        preceded(
            hocon_horizontal_space0,
            alt((
                parse_with_required,
                parse_with_location,
                parse_quoted_string.map(|path| Inclusion::new(path, false, None, None)),
            )),
        ),
    )
    .parse_complete(input)
}

#[inline]
pub(crate) fn parse_include(input: &str) -> R<'_, Inclusion> {
    let (remainder, mut inclusion) = parse.parse_complete(input)?;
    if let Err(error) = parse_inclusion(&mut inclusion) {
        return Err(nom::Err::Failure(crate::parser::HoconParseError::Other(
            error,
        )));
    }
    Ok((remainder, inclusion))
}

fn inclusion_from_file(
    inclusion: &mut Inclusion,
    options: ConfigParseOptions,
) -> crate::Result<()> {
    match load_from_path(&inclusion.path, options) {
        Ok(object) => {
            inclusion.val = Some(object.into());
        }
        e @ Err(crate::error::Error::ConfigNotFound { .. }) => {
            if inclusion.required {
                return e
                    .map(|_| ())
                    .map_err(|e| crate::error::Error::InclusionError {
                        inclusion: inclusion.clone(),
                        error: Box::new(e),
                    });
            }
        }
        Err(e) => {
            return Err(crate::error::Error::InclusionError {
                inclusion: inclusion.clone(),
                error: Box::new(e),
            });
        }
    }
    Ok(())
}

fn inclusion_from_classpath(
    inclusion: &mut Inclusion,
    parse_opts: ConfigParseOptions,
) -> crate::Result<()> {
    match load_from_classpath(&inclusion.path, parse_opts) {
        Ok(object) => {
            inclusion.val = Some(object.into());
        }
        e @ Err(crate::error::Error::ConfigNotFound { .. }) => {
            if inclusion.required {
                return e
                    .map(|_| ())
                    .map_err(|e| crate::error::Error::InclusionError {
                        inclusion: inclusion.clone(),
                        error: Box::new(e),
                    });
            }
        }
        Err(e) => {
            return Err(crate::error::Error::InclusionError {
                inclusion: inclusion.clone(),
                error: Box::new(e),
            });
        }
    }
    Ok(())
}

fn inclusion_from_file_and_classpath(
    inclusion: &mut Inclusion,
    parse_opts: ConfigParseOptions,
) -> crate::Result<()> {
    match loader::load(&inclusion.path, parse_opts) {
        Ok(object) => {
            inclusion.val = Some(object.into());
        }
        e @ Err(crate::error::Error::ConfigNotFound { .. }) => {
            if inclusion.required {
                return e
                    .map(|_| ())
                    .map_err(|e| crate::error::Error::InclusionError {
                        inclusion: inclusion.clone(),
                        error: Box::new(e),
                    });
            }
        }
        Err(e) => {
            return Err(crate::error::Error::InclusionError {
                inclusion: inclusion.clone(),
                error: Box::new(e),
            });
        }
    }
    Ok(())
}

fn inclusion_from_url(
    inclusion: &mut Inclusion,
    parse_opts: ConfigParseOptions,
) -> crate::Result<()> {
    let url = url::Url::from_str(&inclusion.path)?;
    match load_from_url(url, parse_opts) {
        Ok(object) => {
            inclusion.val = Some(object.into());
        }
        e @ Err(crate::error::Error::ConfigNotFound { .. }) => {
            if inclusion.required {
                return e
                    .map(|_| ())
                    .map_err(|e| crate::error::Error::InclusionError {
                        inclusion: inclusion.clone(),
                        error: Box::new(e),
                    });
            }
        }
        Err(e) => {
            return Err(crate::error::Error::InclusionError {
                inclusion: inclusion.clone(),
                error: Box::new(e),
            });
        }
    }
    Ok(())
}

fn parse_inclusion(inclusion: &mut Inclusion) -> crate::Result<()> {
    let mut parse_opts = CONFIG.take();
    if parse_opts
        .includes
        .iter()
        .rfind(|p| p == &&inclusion.path)
        .is_some()
    {
        return Err(crate::error::Error::InclusionCycle(inclusion.path.clone()));
    }
    parse_opts.includes.push(inclusion.path.clone());
    match inclusion.location {
        None | Some(Location::Url) => match url::Url::from_str(&inclusion.path) {
            Ok(url) => {
                if url.scheme() != "file" {
                    inclusion_from_url(inclusion, parse_opts)?;
                }
            }
            _ => {
                inclusion_from_file_and_classpath(inclusion, parse_opts)?;
            }
        },
        Some(Location::Classpath) => inclusion_from_classpath(inclusion, parse_opts)?,
        Some(Location::File) => inclusion_from_file(inclusion, parse_opts)?,
    }
    CONFIG.with_borrow_mut(|c| {
        c.includes.pop();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::parser::include::parse;

    #[rstest]
    #[case(
        "include \"resources/demo.conf\"",
        "include \"resources/demo.conf\"",
        ""
    )]
    #[case("include\"demo.conf\"", "include \"demo.conf\"", "")]
    #[case("include file(\"demo.conf\")", "include file(\"demo.conf\")", "")]
    #[case(
        "include required(\"demo.conf\")",
        "include required(\"demo.conf\")",
        ""
    )]
    #[case(
        "include required( classpath(\"demo.conf\"))",
        "include required(classpath(\"demo.conf\"))",
        ""
    )]
    #[case(
        "include required(url(\"demo.conf\"))",
        "include required(url(\"demo.conf\"))",
        ""
    )]
    #[case(
        "include required( url( \"demo.conf\" ) )",
        "include required(url(\"demo.conf\"))",
        ""
    )]
    #[case(
        "include \"resources/demo.conf\"abc",
        "include \"resources/demo.conf\"",
        "abc"
    )]
    fn test_valid_include(
        #[case] input: &str,
        #[case] expected_result: &str,
        #[case] expected_rest: &str,
    ) {
        let result = parse(input);
        assert!(result.is_ok(), "expected Ok but got {:?}", result);
        let (rest, parsed) = result.unwrap();
        assert_eq!(parsed.to_string(), expected_result);
        assert_eq!(rest, expected_rest);
    }

    #[rstest]
    #[case("include resources/demo.conf")]
    #[case("include required (classpath(\"demo.conf\"))")]
    #[case("include required(classpath (\"demo.conf\"))")]
    #[case("include required (\"demo.conf\")")]
    #[case("included \"demo\"")]
    fn test_invalid_include(#[case] input: &str) {
        let result = parse(input);
        assert!(result.is_err(), "expected Err but got {:?}", result);
    }
}
