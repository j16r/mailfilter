use std::collections::HashMap;
use std::io::{Error, ErrorKind};

//use mailbox::stream::entry::Header;
use mailbox::stream::Entry;
use nom::IResult;
use nom::sequence::{delimited, terminated, tuple};
use nom::bytes::complete::tag;
use nom::character::complete::{anychar, char, multispace1};
use mime::Mime;

use crate::Header;

#[derive(Debug)]
pub struct Mail {
    pub headers: Vec<Header>,
    pub body: HashMap<Mime, Vec<u8>>,
    pub boundary: String
}

#[derive(Debug, Eq, PartialEq)]
pub struct ContentTypeHeader {
    pub mime_type: Mime,
    pub boundary: String,
}

impl Mail {
    pub fn new() -> Mail {
        Mail{
            headers: vec![],
            body: HashMap::new(),
            boundary: "".to_string(),
        }
    }

    pub fn parse(input: &str) -> Result<Mail, std::io::Error> {
        let mut mail: Option<Mail> = None;

        for entry in mailbox::stream::entries(std::io::Cursor::new(input)) {
            match entry {
                Ok(Entry::Begin(_, _)) => {
                    println!("begin...");
                    mail = Some(Mail::new());
                },
                Ok(Entry::Header(ref header)) if &*header.key() == "Content-Type" => {
                    println!("found Content-Type header, saving {:?}", header);
                    let content_type = (&*header.value()).parse::<Mime>().unwrap();
                    if let Some(ref mut m) = mail {
                        m.boundary = content_type.get_param(mime::BOUNDARY).unwrap().as_str().to_string();
                        m.headers.push(header.clone());
                    }
                }
                Ok(Entry::Header(ref header)) => {
                    println!("found header, saving {:?}", header);
                    if let Some(ref mut m) = mail {
                        m.headers.push(header.clone());
                    }
                }
                Ok(Entry::Body(body)) => {
                    println!("body...");
                    if let Some(ref mut m) = mail {
                        if m.boundary == "" {
                            let payload = m.body.entry(mime::TEXT_PLAIN).or_insert(Vec::new());
                            payload.extend(body.iter());
                        }
                    }
                }
                Ok(Entry::End) => {
                    println!("end");
                    if let Some(m) = mail {
                        return Ok(m)
                    }
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::new(ErrorKind::UnexpectedEof, "reached end of buffer before end of email"))
    }

    //pub fn parse_content_type(input: &str) -> IResult<&str, ContentTypeHeader> {
        //let (input, (mime_type, _, _, boundary)) = tuple((
            //terminated(anychar, tag(";")),
            //multispace1,
            //tag("boundary="),
            //delimited(char('"'), anychar, char('"')),
        //))(input)?;
        //Ok((input, ContentTypeHeader{mime_type: mime::TEXT_PLAIN, boundary: boundary.to_string()}))
    //}
}

#[cfg(test)]
mod test {
    use super::*;

    static EMAIL: &'static str = r#"From 1@mail Fri Jun 05 23:22:35 +0000 2020
From: One <1@mail>
Content-Type: multipart/alternative;
 boundary="--_NmP-d4c3c3eca06b99af-Part_1"


----_NmP-d4c3c3eca06b99af-Part_1
Content-Type: text/plain
Content-Transfer-Encoding: quoted-printable

This is en email
----_NmP-d4c3c3eca06b99af-Part_1


"#;

    #[test]
    fn test_parse_empty() {
        assert!(Mail::parse("").is_err());
    }

    #[test]
    fn test_parse_valid_email() {
        let envelope_result = Mail::parse(EMAIL);
        assert!(envelope_result.is_ok());
        let envelope = envelope_result.unwrap();
        assert_eq!(envelope.boundary, "--_NmP-d4c3c3eca06b99af-Part_1".to_string());
        assert_eq!(envelope.headers.len(), 2);
        assert_eq!(&*envelope.headers[0].key(), "From");
        assert_eq!(&*envelope.headers[1].key(), "Content-Type");
        assert_eq!(envelope.body.keys().len(), 1);
        let body = envelope.body.get(&mime::TEXT_PLAIN).unwrap();
        assert_eq!(body, &Vec::<u8>::new());
    }

    //#[test]
    //fn test_parse_content_type() {
        //let (input, header) = Mail::parse_content_type(r#"multipart/alternative;
 //boundary="--_NmP-d4c3c3eca06b99af-Part_1"#).unwrap();
        //assert_eq!(header, ContentTypeHeader{
            //mime_type: Mime::from_string("multipart/alternative"),
            //boundary: "--_NmP-d4c3c3eca06b99af-Part_1".to_string(),
        //});
    //}
}
