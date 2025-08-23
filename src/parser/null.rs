use crate::parser::R;
use nom::Parser;
use nom::bytes::complete::tag;
use nom::error::context;

/// Parses the literal `null` keyword in a HOCON configuration.
///
/// This parser matches the exact string `"null"` at the beginning of the input.
/// On success, it consumes the matched part and returns the remaining input
/// along with the unit value `()`.
///
/// # Parameters
///
/// * `input` - The input string slice to be parsed.
///
/// # Returns
///
/// An [`IResult`] with:
/// - `Ok((remaining, ()))` if `"null"` was successfully parsed.
///   `remaining` is the part of the input left after consuming `"null"`.
/// - `Err(HoconParseError)` if the input does not start with `"null"`.
///
/// # Examples
///
/// ```rust
/// use your_crate::parse_null; // adjust the path to your module
///
/// assert_eq!(parse_null("null"), Ok(("", ())));
/// assert_eq!(parse_null("null rest"), Ok((" rest", ())));
/// assert!(parse_null("none").is_err());
/// ```
///
/// [`IResult`]: nom::IResult
pub(crate) fn parse_null(input: &str) -> R<'_, ()> {
    let (input, _) = context("null", tag("null")).parse(input)?;
    Ok((input, ()))
}
