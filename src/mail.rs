use std::collections::HashMap;
use std::io::{Error, ErrorKind};

use mailbox::stream::Entry;
use mime::Mime;

use crate::Header;

#[derive(Debug)]
pub struct Mail {
    pub headers: Vec<Header>,
    pub body: HashMap<Mime, Vec<u8>>,
    pub boundary: String,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ContentTypeHeader {
    pub mime_type: Mime,
    pub boundary: String,
}

impl Mail {
    pub fn new() -> Mail {
        Mail {
            headers: vec![],
            body: HashMap::new(),
            boundary: "".to_string(),
        }
    }

    pub fn parse(input: &str) -> Result<Mail, std::io::Error> {
        let mut mail: Option<Mail> = None;
        let mut reading_headers = false;
        let mut reading_body = false;
        let mut current_body: Option<Mime> = None;

        for entry in mailbox::stream::entries(std::io::Cursor::new(input)) {
            match entry {
                Ok(Entry::Begin(_, _)) => {
                    mail = Some(Mail::new());
                }
                Ok(Entry::Header(ref header)) if &*header.key() == "Content-Type" => {
                    let content_type = (&*header.value()).parse::<Mime>().unwrap();
                    if let Some(ref mut m) = mail {
                        m.boundary = format!(
                            "--{}",
                            content_type
                                .get_param(mime::BOUNDARY)
                                .unwrap()
                                .as_str()
                                .to_string()
                        );
                        m.headers.push(header.clone());
                    }
                }
                Ok(Entry::Header(ref header)) => {
                    if let Some(ref mut m) = mail {
                        m.headers.push(header.clone());
                    }
                }
                Ok(Entry::Body(body)) => {
                    let body_string = std::str::from_utf8(&body).unwrap();
                    if let Some(ref mut m) = mail {
                        if m.boundary == "" {
                            let payload = m.body.entry(mime::TEXT_PLAIN).or_insert(Vec::new());
                            payload.extend(body.iter());
                        } else {
                            if reading_body {
                                if body_string == m.boundary {
                                    reading_body = false;
                                } else if let Some(ref mime_type) = current_body {
                                    let payload =
                                        m.body.entry(mime_type.clone()).or_insert(Vec::new());
                                    payload.extend(body.iter());
                                }
                            }

                            if reading_headers {
                                if body_string == "" {
                                    reading_headers = false;
                                    reading_body = true;
                                } else {
                                    let header = Header::new(body_string).unwrap();
                                    if &*header.key() == "Content-Type" {
                                        let mime_type = (&*header.value()).parse::<Mime>().unwrap();
                                        m.body.entry(mime_type.clone()).or_insert(Vec::new());
                                        current_body = Some(mime_type.clone());
                                    }
                                }
                            } else if m.boundary == body_string {
                                reading_headers = true;
                                reading_body = false;
                            }
                        }
                    }
                }
                Ok(Entry::End) => {
                    if let Some(m) = mail {
                        return Ok(m);
                    }
                }
                Err(e) => return Err(e),
            }
        }

        Err(Error::new(
            ErrorKind::UnexpectedEof,
            "reached end of buffer before end of email",
        ))
    }
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

This is an email
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
        assert_eq!(
            envelope.boundary,
            "----_NmP-d4c3c3eca06b99af-Part_1".to_string()
        );
        assert_eq!(envelope.headers.len(), 2);
        assert_eq!(&*envelope.headers[0].key(), "From");
        assert_eq!(&*envelope.headers[1].key(), "Content-Type");
        assert_eq!(envelope.body.keys().len(), 1);
        let body = envelope.body.get(&mime::TEXT_PLAIN).unwrap();
        assert_eq!(
            std::str::from_utf8(body).unwrap(),
            "This is an email".to_string()
        );
    }
}
