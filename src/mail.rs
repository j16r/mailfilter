use std::collections::HashMap;
use std::io::{Error, ErrorKind};

use mailbox::stream::Entry;
use mime::Mime;
use thiserror::Error;

use crate::Header;

#[derive(Debug)]
pub struct Mail {
    pub headers: Vec<Header>,
    pub body: HashMap<Mime, Vec<u8>>,
    pub boundary: String,
}

impl Mail {
    pub fn body_text(&self) -> String {
        for (key, value) in self.body.iter() {
            if mime::TEXT_PLAIN.essence_str() == key.essence_str() {
                return std::str::from_utf8(value).unwrap().to_string();
            }
        }
        "".to_string()
    }

    pub fn subject(&self) -> String {
        for header in self.headers.iter() {
            if &*header.key() == "Subject" {
                return header.value().to_string();
            }
        }
        "".to_string()
    }

    pub fn date(&self) -> String {
        for header in self.headers.iter() {
            if &*header.key() == "Date" {
                if let Ok(date) = chrono::DateTime::parse_from_rfc2822(&header.value()) {
                    // , "%Y-%m-%d") {
                    return date.format("%Y%m%dT%H%M%S").to_string();
                }
                return header.value().to_string();
            }
        }
        "".to_string()
    }
}

#[derive(Error, Debug)]
pub enum ContentTypeError {
    #[error(transparent)]
    TypeError(#[from] mime::FromStrError),
    #[error("failed to parse Content-type header")]
    ValueError,
}

fn parse_content_type_header(header_value: &str) -> Result<Mime, ContentTypeError> {
    if let Ok(mime_type) = header_value.parse::<Mime>() {
        return Ok(mime_type);
    }
    // Fallback, eliminate anything after the first ';'
    match header_value.split(';').collect::<Vec<_>>()[..] {
        [parts, ..] if parts == "text" => Ok(mime::TEXT_PLAIN),
        [parts, ..] => Ok(parts.parse::<Mime>()?),
        _ => Err(ContentTypeError::ValueError),
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ContentTypeHeader {
    pub mime_type: Mime,
    pub boundary: String,
}

#[derive(Default)]
pub struct Context {
    mail: Option<Mail>,
    reading_headers: bool,
    reading_body: bool,
    current_body: Option<Mime>,
}

impl Context {
    pub fn new() -> Context {
        Context {
            ..Context::default()
        }
    }

    pub fn begin(&mut self) {
        self.mail = Some(Mail::new());
    }

    pub fn end(&mut self) -> Option<Mail> {
        self.mail.take()
    }

    pub fn header(&mut self, header: &Header) {
        if let Some(ref mut m) = self.mail {
            if &*header.key() == "Content-Type" {
                if let Ok(content_type) = parse_content_type_header(&*header.value()) {
                    if let Some(ref boundary) = content_type.get_param(mime::BOUNDARY) {
                        m.boundary = format!("--{}", boundary.as_str());
                    }
                }
            } else {
            }
            m.headers.push(header.clone());
        }
    }

    pub fn body(&mut self, body: &[u8]) {
        if let Some(ref mut m) = self.mail {
            if m.boundary.is_empty() {
                let payload = m.body.entry(mime::TEXT_PLAIN).or_insert_with(Vec::new);
                payload.extend(body.iter());
                payload.extend(b"\n");
            } else {
                let body_string = std::str::from_utf8(body).unwrap();
                if self.reading_body {
                    if body_string == m.boundary {
                        self.reading_body = false;
                    } else if let Some(ref mime_type) = self.current_body {
                        let payload = m.body.entry(mime_type.clone()).or_insert_with(Vec::new);
                        payload.extend(body.iter());
                        payload.extend(b"\n");
                    }
                }

                if self.reading_headers {
                    if body_string.is_empty() {
                        self.reading_headers = false;
                        self.reading_body = true;
                    } else if let Ok(header) = Header::new(body_string) {
                        if &*header.key() == "Content-Type" {
                            if let Ok(mime_type) = parse_content_type_header(&*header.value()) {
                                m.body.entry(mime_type.clone()).or_insert_with(Vec::new);
                                self.current_body = Some(mime_type);
                            } else {
                                eprintln!("Unrecognized mime type: {}", &*header.value());
                            }
                        }
                    }
                } else if m.boundary == body_string {
                    self.reading_headers = true;
                    self.reading_body = false;
                }
            }
        }
    }
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
        let mut ctx = Context::new();

        for entry in mailbox::stream::entries(std::io::Cursor::new(input)) {
            match entry {
                Ok(Entry::Begin(_, _)) => {
                    ctx.begin();
                }
                Ok(Entry::Header(ref header)) => {
                    ctx.header(header);
                }
                Ok(Entry::Body(ref body)) => {
                    ctx.body(body);
                }
                Ok(Entry::End) => {
                    if let Some(m) = ctx.end() {
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

    static EMAIL: &str = r#"From 1@mail Fri Jun 05 23:22:35 +0000 2020
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
        assert_eq!(body, b"This is an email\n");
    }

    #[test]
    fn test_parse_content_type_header() {
        assert_eq!(
            parse_content_type_header("text/html").unwrap(),
            mime::TEXT_HTML
        );
        assert_eq!(
            parse_content_type_header("text/html; garbage").unwrap(),
            mime::TEXT_HTML
        );
        assert_eq!(parse_content_type_header("text").unwrap(), mime::TEXT_PLAIN);
        let multipart = parse_content_type_header(
            r#"multipart/alternative; boundary="--_NmP-d4c3c3eca06b99af-Part_1""#,
        )
        .unwrap();
        assert_eq!(
            multipart.get_param(mime::BOUNDARY).unwrap(),
            "--_NmP-d4c3c3eca06b99af-Part_1"
        );
    }
}
