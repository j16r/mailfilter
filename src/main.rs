extern crate mailbox;
extern crate regex;

use regex::Regex;
use std::borrow::Cow;
use std::env;
use std::fs::File;
use std::io::Write;
use mailbox::stream::Entry::{Body, End, Header};

fn main() {
    let path = env::args().nth(1).expect("no file given");

    let mut file : Option<File> = None;

    for entry in mailbox::stream::entries(File::open(path).unwrap()) {
        match entry {
            Ok(Header(ref header)) if &*header.key() == "Subject" && header.value().contains("John Barker") => {
                println!("{:?}", header.value());

                let subject = &header.value();
                let name = envelope_filename(subject);
                let path = format!("{}.txt", &name);
                file = Some(File::create(&path).unwrap());
            },
            Ok(End) => {
                if let Some(ref f) = file {
                    f.sync_all().unwrap();
                    file = None;
                }
            },
            Ok(Body(body)) => {
                if let Some(ref mut f) = file {
                    f.write_all(&body).unwrap();
                }
            },
            _ => {}
        }
    }
}

fn envelope_filename<'a>(path: &'a str) -> Cow<'a, str> {
    let filename_regex = Regex::new(r"[^A-Za-z0-9]+").unwrap();
    filename_regex.replace_all(path, "_")
}
