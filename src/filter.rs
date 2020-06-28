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

#[derive(Debug, Eq, PartialEq)]
pub struct Matcher {
    key: String,
    value_matcher: ValueMatcher,
}

impl Matcher {
    pub fn matches(&self, header: &Header) -> bool {
        &*header.key() == self.key
            && match self.value_matcher {
                ValueMatcher::StartsWith(ref beginning) => header.value().starts_with(beginning),
                ValueMatcher::EndsWith(ref end) => header.value().ends_with(end),
                ValueMatcher::Exact(ref string) => &*header.value() == string,
                ValueMatcher::Regex(ref matching_regex) => matching_regex.is_match(&header.value()),
                ValueMatcher::NotEqual(ref string) => &*header.value() != string,
                ValueMatcher::NotRegex(ref matching_regex) => {
                    !matching_regex.is_match(&header.value())
                }
            }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Expression {
    Matcher(Matcher),
    Or(Matcher, Box<Expression>),
    And(Matcher, Box<Expression>),
}

impl Expression {
    pub fn matches(&self, header: &Header) -> bool {
        true
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Filter {
    pub expression: Option<Expression>,
}

impl Filter {
    pub fn matches(&self, header: &Header) -> bool {
        if let Some(ref expr) = self.expression {
            return expr.matches(header);
        }
        true
    }
}

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
            key: key.to_string(),
            value_matcher,
        },
    ))
}

fn value_matcher(input: &str) -> IResult<&str, ValueMatcher> {
    let (input, (operator, argument)) = alt((
        tuple((tag("=~"), regex)),
        tuple((tag("!~"), regex)),
        tuple((tag("=^"), literal)),
        tuple((tag("=$"), literal)),
        tuple((tag("!="), literal)),
        tuple((tag("="), literal)),
    ))(input)?;
    let matcher = match operator {
        "=" => ValueMatcher::Exact(argument.to_string()),
        "=^" => ValueMatcher::StartsWith(argument.to_string()),
        "=$" => ValueMatcher::EndsWith(argument.to_string()),
        "!=" => ValueMatcher::NotEqual(argument.to_string()),
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
    Ok((input, regex.to_string()))
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
    fn test_parse_single_matcher() {
        assert_eq!(
            parse("subject=hello").unwrap(),
            (
                "",
                Filter {
                    expression: Some(Expression::Matcher(Matcher {
                        key: "subject".to_string(),
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
                        key: "subject".to_string(),
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
                            key: "subject".to_string(),
                            value_matcher: ValueMatcher::Regex(
                                Regex::new("this / then that").unwrap()
                            ),
                        },
                        Box::new(Expression::Matcher(Matcher {
                            key: "body".to_string(),
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
