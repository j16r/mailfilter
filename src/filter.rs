use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use mime::Mime;
use nom::branch::alt;
use nom::bytes::complete::{escaped, tag, tag_no_case, take_while};
use nom::character::complete::{alphanumeric1, char, multispace1, none_of, one_of};
use nom::error::ParseError;
use nom::sequence::delimited;
use nom::sequence::tuple;
use nom::IResult;
use regex::Regex;
use regex_syntax::Parser;
use yz_nomstr::parse_string;

use crate::Header;
use crate::Mail;

#[derive(Debug)]
pub enum ValueMatcher {
    Exact(String),
    StartsWith(String),
    EndsWith(String),
    Regex(Regex),
    NotEqual(String),
    NotRegex(Regex),
}

impl PartialEq for ValueMatcher {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ValueMatcher::Exact(ref lhs), ValueMatcher::Exact(ref rhs)) => lhs == rhs,
            (ValueMatcher::StartsWith(ref lhs), ValueMatcher::StartsWith(ref rhs)) => lhs == rhs,
            (ValueMatcher::EndsWith(ref lhs), ValueMatcher::EndsWith(ref rhs)) => lhs == rhs,
            (ValueMatcher::Regex(ref lhs), ValueMatcher::Regex(ref rhs)) => {
                format!("{}", lhs) == format!("{}", rhs)
            }
            (ValueMatcher::NotEqual(ref lhs), ValueMatcher::NotEqual(ref rhs)) => lhs == rhs,
            (ValueMatcher::NotRegex(ref lhs), ValueMatcher::NotRegex(ref rhs)) => {
                format!("{}", lhs) == format!("{}", rhs)
            }
            _ => false,
        }
    }
}

impl Eq for ValueMatcher {}

impl ValueMatcher {
    pub fn matches(&self, value: &str) -> bool {
        match self {
            ValueMatcher::StartsWith(ref beginning) => value.starts_with(beginning),
            ValueMatcher::EndsWith(ref end) => value.ends_with(end),
            ValueMatcher::Exact(ref string) => value == string,
            ValueMatcher::Regex(ref matching_regex) => matching_regex.is_match(value),
            ValueMatcher::NotEqual(ref string) => value != string,
            ValueMatcher::NotRegex(ref matching_regex) => !matching_regex.is_match(value),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum MatcherKey {
    BodyMatcher(Mime),
    HeaderMatcher(String),
}

impl MatcherKey {
    fn new(input: &str) -> Result<MatcherKey, mime::FromStrError> {
        let body_matcher = Regex::new(r"^body(?:[.](.*))?$").unwrap();
        if let Some(captures) = body_matcher.captures(input) {
            if captures.len() == 1 {
                return Ok(MatcherKey::BodyMatcher(
                    captures[1].parse::<Mime>().unwrap(),
                ));
            } else {
                return Ok(MatcherKey::BodyMatcher(
                    "text/plain".parse::<Mime>().unwrap(),
                ));
            }
        }
        Ok(MatcherKey::HeaderMatcher(input.to_string()))
    }

    fn is_header(&self, header: &Header) -> bool {
        if let MatcherKey::HeaderMatcher(ref key) = self {
            return header.key().eq_ignore_ascii_case(key);
        }
        false
    }

    fn get_matching_body(&self, body: &HashMap<Mime, Vec<u8>>) -> Option<String> {
        if let MatcherKey::BodyMatcher(ref mime_type) = self {
            for (key, value) in body.iter() {
                if mime_type.essence_str() == key.essence_str() {
                    return Some(std::str::from_utf8(value).unwrap().to_string());
                }
            }
        }
        None
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Matcher {
    key: MatcherKey,
    value_matcher: ValueMatcher,
}

impl Matcher {
    pub fn includes_header(&self, header: &Header) -> bool {
        self.key.is_header(header)
    }

    pub fn matches(&self, mail: &Mail) -> bool {
        match self.key {
            MatcherKey::BodyMatcher(ref mime_type) => self.matches_body(mime_type, &mail.body),
            MatcherKey::HeaderMatcher(_) => self.matches_header(&mail.headers),
        }
    }

    fn matches_body(&self, _mime_type: &Mime, body: &HashMap<Mime, Vec<u8>>) -> bool {
        if let Some(body_text) = self.key.get_matching_body(body) {
            return self.value_matcher.matches(&body_text);
        }
        false
    }

    fn matches_header(&self, headers: &[Header]) -> bool {
        !headers.is_empty()
            && headers
                .iter()
                .filter(|header| -> bool { self.key.is_header(header) })
                .any(|header| -> bool { self.value_matcher.matches(&*header.value()) })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Expression {
    Matcher(Matcher),
    Or(Matcher, Box<Expression>),
    And(Matcher, Box<Expression>),
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Matcher(ref matcher) => write!(f, "{:?}", matcher),
            Expression::Or(ref matcher, ref expression) => {
                write!(f, "{:?} or {:?}", matcher, expression)
            }
            Expression::And(ref matcher, ref expression) => {
                write!(f, "{:?} and {:?}", matcher, expression)
            }
        }
    }
}

impl Expression {
    // detect if any part of the expression mentions this header
    pub fn includes_header(&self, header: &Header) -> bool {
        match self {
            Expression::Matcher(ref matcher) => matcher.includes_header(header),
            Expression::Or(ref matcher, ref expression) => {
                matcher.includes_header(header) || expression.includes_header(header)
            }
            Expression::And(ref matcher, ref expression) => {
                matcher.includes_header(header) || expression.includes_header(header)
            }
        }
    }

    pub fn matches(&self, header: &Mail) -> bool {
        match self {
            Expression::Matcher(ref matcher) => matcher.matches(header),
            Expression::Or(ref matcher, ref expression) => {
                matcher.matches(header) || expression.matches(header)
            }
            Expression::And(ref matcher, ref expression) => {
                matcher.matches(header) && expression.matches(header)
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Filter {
    pub expression: Option<Expression>,
}

pub const ANY: Filter = Filter { expression: None };

impl Filter {
    // detect if this header is mentioned at all in the filter
    pub fn includes_header(&self, header: &Header) -> bool {
        self.expression
            .as_ref()
            .map(|e| e.includes_header(header))
            .unwrap_or(true)
    }

    // detect if this mail matches the filter
    pub fn matches(&self, mail: &Mail) -> bool {
        self.expression
            .as_ref()
            .map(|e| e.matches(mail))
            .unwrap_or(true)
    }
}

impl FromStr for Filter {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.is_ascii() {
            return Ok(ANY);
        }
        dbg!(&input);
        match expression(input) {
            Ok((_, expression)) => Ok(Filter {
                expression: Some(expression),
            }),
            Err(e) => Err(e.to_string()),
        }
    }
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.expression {
            Some(ref e) => write!(f, "{}", e),
            None => write!(f, ""),
        }
    }
}

#[cfg(test)]
pub fn parse(input: &str) -> IResult<&str, Filter> {
    let (input, expression) = expression(input)?;
    Ok((
        input,
        Filter {
            expression: Some(expression),
        },
    ))
}

fn expression(input: &str) -> IResult<&str, Expression> {
    alt((and_expression, or_expression, match_expression))(input)
}

fn match_expression(input: &str) -> IResult<&str, Expression> {
    let (input, matcher) = matcher(input)?;
    Ok((input, Expression::Matcher(matcher)))
}

fn or_expression(input: &str) -> IResult<&str, Expression> {
    let (input, (matcher, _, _, _, right_expression)) = tuple((
        matcher,
        multispace1,
        tag_no_case("or"),
        multispace1,
        expression,
    ))(input)?;
    Ok((input, Expression::Or(matcher, Box::new(right_expression))))
}

fn and_expression(input: &str) -> IResult<&str, Expression> {
    let (input, (matcher, _, _, _, right_expression)) = tuple((
        matcher,
        multispace1,
        tag_no_case("and"),
        multispace1,
        expression,
    ))(input)?;
    Ok((input, Expression::And(matcher, Box::new(right_expression))))
}

fn matcher(input: &str) -> IResult<&str, Matcher> {
    let (input, (key, value_matcher)) = tuple((key, value_matcher))(input)?;
    Ok((
        input,
        Matcher {
            key: MatcherKey::new(key).unwrap(),
            value_matcher,
        },
    ))
}

fn value_matcher(input: &str) -> IResult<&str, ValueMatcher> {
    let (input, (operator, argument)) = alt((
        tuple((tag("=~"), regex)),
        tuple((tag("!~"), regex)),
        tuple((tag("^="), literal)),
        tuple((tag("$="), literal)),
        tuple((tag("!="), literal)),
        tuple((tag("="), literal)),
    ))(input)?;
    let matcher = match operator {
        "=" => ValueMatcher::Exact(argument),
        "^=" => ValueMatcher::StartsWith(argument),
        "$=" => ValueMatcher::EndsWith(argument),
        "!=" => ValueMatcher::NotEqual(argument),
        "=~" => {
            let regex = Regex::new(&argument).unwrap();
            ValueMatcher::Regex(regex)
        }
        "!~" => {
            let regex = Regex::new(&argument).unwrap();
            ValueMatcher::NotRegex(regex)
        }
        _ => unreachable!("unrecognized match arm for operator {:?}", operator),
    };
    Ok((input, matcher))
}

fn literal<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, String, E> {
    alt((alphanumeric_literal, quoted_string))(input)
}

fn alphanumeric_literal<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, String, E> {
    let (input, value) = alphanumeric1(input)?;
    Ok((input, value.to_string()))
}

fn quoted_string<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, String, E> {
    let parser = parse_string::<_, E>('"');
    let (input, bytes) = parser(input)?;
    let string = std::str::from_utf8(bytes.as_ref()).unwrap();
    Ok((input, string.to_string()))
}

fn parse_regex(input: &str) -> IResult<&str, String> {
    let (input, regex) = escaped(none_of("/\\"), '\\', one_of("/"))(input)?;
    let unescaped_regex = regex.to_string().replace("\\/", "/");
    match Parser::new().parse(&unescaped_regex) {
        Ok(_) => Ok((input, unescaped_regex)),
        Err(e) => panic!("invalid regex {:?}", e),
    }
}

fn regex(input: &str) -> IResult<&str, String> {
    let (input, regex) = delimited(char('/'), parse_regex, char('/'))(input)?;
    Ok((input, regex))
}

fn key(input: &str) -> IResult<&str, &str> {
    take_while(is_printable)(input)
}

fn is_printable(ch: char) -> bool {
    ch.is_ascii_alphabetic()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_example() {
        let (_, program) = parse("subject=~/check off$/ or body=~/body/").unwrap();
        let envelope = Mail::parse(
            r#"From 1@mail Fri Jun 05 23:22:35 +0000 2020
From: A Person <me@readme.com>
Subject: Items to check off
To: You <you@readme.com>
Content-Type: multipart/alternative; boundary="0000000000006e22da05a4839fb9"

--0000000000006e22da05a4839fb9
Content-Type: text/plain; charset="UTF-8"

Email body goes here.
--0000000000006e22da05a4839fb9
Content-Type: text/html; charset="UTF-8"
Content-Transfer-Encoding: quoted-printable

<div>Hello!</div>
--0000000000006e22da05a4839fb9

"#,
        )
        .unwrap();

        assert!(program.matches(&envelope));
    }

    #[test]
    fn test_empty_program_returns_all_envelopes() {
        let program = Filter { expression: None };
        let envelope = Mail::parse(
            r#"From 1@mail Fri Jun 05 23:22:35 +0000 2020
From: One <1@mail>


"#,
        )
        .unwrap();

        assert!(program.matches(&envelope));
    }

    #[test]
    fn test_includes_header() {
        let (_, program) = parse("From=One").unwrap();
        let envelope = Mail::parse(
            r#"From 1@mail Fri Jun 05 23:22:35 +0000 2020
From: One <1@mail>


"#,
        )
        .unwrap();

        assert!(program.includes_header(&envelope.headers[0]));

        let (_, non_matching_program) = parse("subject=Hello").unwrap();
        assert!(!non_matching_program.includes_header(&envelope.headers[0]));
    }

    #[test]
    fn test_includes_header_empty_email_fails_match() {
        let envelope = Mail::new();

        let (_, program) = parse("From=One").unwrap();
        assert!(!program.matches(&envelope));

        let (_, program) = parse("From=One and subject=dude").unwrap();
        assert!(!program.matches(&envelope));
    }

    #[test]
    fn test_parse_single_matcher() {
        assert_eq!(
            parse("subject=hello").unwrap(),
            (
                "",
                Filter {
                    expression: Some(Expression::Matcher(Matcher {
                        key: MatcherKey::new("subject").unwrap(),
                        value_matcher: ValueMatcher::Exact("hello".to_string()),
                    }))
                }
            )
        );
    }

    #[test]
    fn test_parse_single_regex() {
        assert_eq!(
            parse("subject=~/^hello$/").unwrap(),
            (
                "",
                Filter {
                    expression: Some(Expression::Matcher(Matcher {
                        key: MatcherKey::new("subject").unwrap(),
                        value_matcher: ValueMatcher::Regex(Regex::new("^hello$").unwrap()),
                    }))
                }
            )
        );
    }

    #[test]
    fn test_parse_regex_or_starts_with() {
        assert_eq!(
            parse("subject=~/this \\/ then that/ or body^=Dear").unwrap(),
            (
                "",
                Filter {
                    expression: Some(Expression::Or(
                        Matcher {
                            key: MatcherKey::new("subject").unwrap(),
                            value_matcher: ValueMatcher::Regex(
                                Regex::new("this / then that").unwrap()
                            ),
                        },
                        Box::new(Expression::Matcher(Matcher {
                            key: MatcherKey::new("body").unwrap(),
                            value_matcher: ValueMatcher::StartsWith("Dear".to_string()),
                        })),
                    ),)
                }
            )
        );
    }

    #[test]
    fn test_quoted_string_empty() {
        assert_eq!(quoted_string::<()>(r#""""#).unwrap(), ("", "".to_string()));
    }

    #[test]
    fn test_quoted_string_whitespace() {
        assert_eq!(
            quoted_string::<()>(r#"" ""#).unwrap(),
            ("", " ".to_string())
        );
    }

    #[test]
    fn test_quoted_string_single_quote() {
        assert_eq!(
            quoted_string::<()>(r#""a \" b""#).unwrap(),
            ("", "a \" b".to_string())
        );
    }

    #[test]
    fn test_quoted_string_unicode() {
        assert_eq!(
            quoted_string::<()>(r#""a \u{1F602} b""#).unwrap(),
            ("", "a 😂 b".to_string())
        );
    }

    #[test]
    fn test_quoted_string_multiple_quotes() {
        assert_eq!(
            quoted_string::<()>(r#""x \" y \" z""#).unwrap(),
            ("", "x \" y \" z".to_string())
        );
    }

    #[test]
    fn test_regex_empty() {
        assert!(regex(r"//").is_err());
    }

    #[test]
    fn test_regex_group() {
        assert_eq!(regex(r"/(.*)/").unwrap(), ("", "(.*)".to_string()));
    }

    #[test]
    fn test_regex_escaped_slash() {
        assert_eq!(regex(r"/\//").unwrap(), ("", r"/".to_string()));
    }

    #[test]
    fn test_regex_escaped_slash_and_whitespace() {
        assert_eq!(regex(r"/\/ \//").unwrap(), ("", r"/ /".to_string()));
    }

    #[test]
    fn test_regex_slash_embedded_class() {
        assert_eq!(regex(r"/[\/]/").unwrap(), ("", r"[/]".to_string()));
    }
}
