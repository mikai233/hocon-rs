use crate::parser::string::parse_quoted_string;
use crate::parser::{hocon_horizontal_multi_space0, parse, R};
use crate::raw::include::{Inclusion, Location};
use crate::raw::raw_object::RawObject;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::char;
use nom::combinator::value;
use nom::sequence::{delimited, preceded};
use nom::Parser;

fn parse_with_location(input: &str) -> R<'_, Inclusion> {
    let (remainder, (location, path)) = (
        alt(
            (
                value(Location::File, tag("file")),
                #[cfg(feature = "url")]
                value(Location::Url, tag("url")),
                value(Location::Classpath, tag("classpath"))
            )
        ),
        delimited(
            char('('),
            delimited(
                hocon_horizontal_multi_space0,
                parse_quoted_string,
                hocon_horizontal_multi_space0,
            ),
            char(')'),
        )
    ).parse_complete(input)?;
    let inclusion = Inclusion::new(path, false, Some(location), None);
    Ok((remainder, inclusion))
}

fn parse_with_required(input: &str) -> R<'_, Inclusion> {
    delimited(
        tag("required("),
        delimited(
            hocon_horizontal_multi_space0,
            alt(
                (
                    parse_with_location.map(|mut inclusion| {
                        inclusion.required = true;
                        inclusion
                    }),
                    parse_quoted_string.map(|path| {
                        Inclusion::new(path, true, None, None)
                    })
                )
            ),
            hocon_horizontal_multi_space0,
        ),
        tag(")"),
    ).parse_complete(input)
}

pub(crate) fn parse_include(input: &str) -> R<'_, Inclusion> {
    let (remainder, mut inclusion) = preceded(
        tag("include"),
        preceded(
            hocon_horizontal_multi_space0,
            alt(
                (
                    parse_with_required,
                    parse_with_location,
                    parse_quoted_string.map(|path| {
                        Inclusion::new(path, false, None, None)
                    }),
                )
            ),
        ),
    ).parse_complete(input)?;
    parse_inclusion(&mut inclusion);
    Ok((remainder, inclusion))
}

fn parse_inclusion(inclusion: &mut Inclusion) {
    match inclusion.location {
        None => {
            todo!()
        }
        Some(location) => {
            match location {
                Location::File => {
                    if inclusion.path.ends_with(".conf") {
                        match std::fs::read_to_string(&inclusion.path) {
                            Ok(data) => {
                                let (_, object) = parse(&data).unwrap();
                                inclusion.val = Some(object.into());
                            }
                            Err(_) => {
                                if inclusion.required {
                                    panic!("required file not found");
                                }
                            }
                        }
                    } else {
                        todo!()
                    }
                }
                #[cfg(feature = "url")]
                Location::Url => {}
                Location::Classpath => {}
            }
        }
    }
}

trait IncludeResolver {
    fn resolve(&self) -> crate::Result<RawObject>;
}

#[derive(Debug)]
struct JsonIncludeResolver<'a> {
    json_string: &'a str,
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