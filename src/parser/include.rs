use std::str::FromStr;

use crate::parser::loader::{load_from_file, load_from_url};
use crate::parser::string::parse_quoted_string;
use crate::parser::{R, hocon_horizontal_multi_space0};
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
            #[cfg(feature = "url")]
            value(Location::Url, tag("url")),
            value(Location::Classpath, tag("classpath")),
        )),
        delimited(
            char('('),
            delimited(
                hocon_horizontal_multi_space0,
                parse_quoted_string,
                hocon_horizontal_multi_space0,
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
            hocon_horizontal_multi_space0,
            alt((
                parse_with_location.map(|mut inclusion| {
                    inclusion.required = true;
                    inclusion
                }),
                parse_quoted_string.map(|path| Inclusion::new(path, true, None, None)),
            )),
            hocon_horizontal_multi_space0,
        ),
        tag(")"),
    )
    .parse_complete(input)
}

pub(crate) fn parse_include(input: &str) -> R<'_, Inclusion> {
    let (remainder, mut inclusion) = preceded(
        tag("include"),
        preceded(
            hocon_horizontal_multi_space0,
            alt((
                parse_with_required,
                parse_with_location,
                parse_quoted_string.map(|path| Inclusion::new(path, false, None, None)),
            )),
        ),
    )
    .parse_complete(input)?;
    parse_inclusion(&mut inclusion).unwrap();
    Ok((remainder, inclusion))
}

fn inclusion_from_file(inclusion: &mut Inclusion) -> crate::Result<()> {
    match load_from_file(&inclusion.path, None) {
        Ok(object) => {
            inclusion.val = Some(object.into());
        }
        Err(error) => {
            if inclusion.required {
                panic!("required file not found");
            } else {
                return Err(error);
            }
        }
    }
    Ok(())
}

fn inclusion_from_url(inclusion: &mut Inclusion) -> crate::Result<()> {
    let url = url::Url::from_str(&inclusion.path)?;
    match load_from_url(url, None) {
        Ok(object) => {
            inclusion.val = Some(object.into());
        }
        Err(error) => {
            if inclusion.required {
                panic!("required url not found");
            } else {
                return Err(error);
            }
        }
    }
    Ok(())
}

fn parse_inclusion(inclusion: &mut Inclusion) -> crate::Result<()> {
    match inclusion.location {
        None => {
            let url = url::Url::from_str(&inclusion.path)?;
            if url.scheme() == "file" {
                inclusion_from_file(inclusion)?;
            } else {
                inclusion_from_url(inclusion)?;
            }
        }
        Some(location) => match location {
            Location::File => inclusion_from_file(inclusion)?,
            #[cfg(feature = "url")]
            Location::Url => inclusion_from_url(inclusion)?,
            Location::Classpath => {}
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::parser::include::parse_include;
    use crate::raw::include::Location;

    #[test]
    fn test_parse_include() {
        let (_, i) = parse_include("include \"demo.conf\"").unwrap();
        assert_eq!(i.path, "demo.conf");
        assert_eq!(i.required, false);
        let (_, i) = parse_include("include\"demo.conf\"").unwrap();
        assert_eq!(i.path, "demo.conf");
        assert_eq!(i.required, false);
        let (_, i) = parse_include("include file(\"demo.conf\")").unwrap();
        assert_eq!(i.path, "demo.conf");
        assert_eq!(i.required, false);
        assert_eq!(i.location, Some(Location::File));
        let (_, i) = parse_include("include required(\"demo.conf\")").unwrap();
        assert_eq!(i.path, "demo.conf");
        assert_eq!(i.required, true);
        assert_eq!(i.location, None);
        let (_, i) = parse_include("include required( classpath(\"demo.conf\"))").unwrap();
        assert_eq!(i.path, "demo.conf");
        assert_eq!(i.required, true);
        assert_eq!(i.location, Some(Location::Classpath));
    }
}
