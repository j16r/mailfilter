extern crate chrono;
extern crate clap;
extern crate mailbox;
extern crate mime;
extern crate nom;
extern crate regex;
extern crate regex_syntax;
extern crate yz_nomstr;

mod filter;
mod mail;

use std::borrow::Cow;
use std::fs::File;
use std::io::Write;

use clap::{App, Arg, SubCommand};
use mailbox::stream::entry::Header;
use mailbox::stream::Entry;
use regex::Regex;

use filter::Filter;
use mail::{Context, Mail};

fn main() {
    let matches = App::new("Mailfilter")
        .about("Process mbox format files")
        .subcommand(
            SubCommand::with_name("extract")
                .about("Extract individual messages")
                .arg(Arg::with_name("file").takes_value(true).required(true))
                .arg(Arg::with_name("filter").takes_value(true)),
        )
        .subcommand(SubCommand::with_name("count").about("Count how many messages match"))
        .get_matches();

    if let Some(command) = matches.subcommand_matches("extract") {
        if let Some(path) = command.value_of("file") {
            eprint!("Extracting envelopes...\nInput:\t{}", path);

            let filter = match command.value_of("filter") {
                Some(filter) => {
                    eprintln!("\nFilter:\t{}", filter);
                    filter::parse(filter).unwrap().1
                }
                _ => Filter { expression: None },
            };

            eprintln!("");

            if let Err(e) = extract(path, &filter) {
                eprintln!("{:?}", e);
            }
        } else {
            eprintln!("No file specified");
        }
    } else {
        eprintln!("No command specified");
    }
}

fn extract(path: &str, filter: &Filter) -> Result<(), std::io::Error> {
    let mut ctx = Context::new();

    for entry in mailbox::stream::entries(File::open(path)?) {
        match entry {
            Ok(Entry::Begin(_, _)) => {
                ctx.begin();
            }
            Ok(Entry::Header(ref header)) => {
                ctx.header(header);
            }
            Ok(Entry::Body(body)) => {
                ctx.body(&body);
            }
            Ok(Entry::End) => {
                if let Some(ref m) = ctx.end() {
                    if filter.matches(m) {
                        let date = m.date();
                        let subject = m.subject();
                        let base_name = format!("{}-{}", date, subject);
                        let name = envelope_filename(&base_name);
                        let path = format!("{}.txt", &name);
                        eprintln!("Saving email to {}", path);
                        let mut file = File::create(&path).unwrap();
                        let body_text = m.body_text();
                        file.write_all(&body_text.into_bytes()).unwrap();
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn envelope_filename<'a>(path: &'a str) -> Cow<'a, str> {
    let filename_regex = Regex::new(r"[^A-Za-z0-9]+").unwrap();
    let sanitized_path = filename_regex.replace_all(path, "_").trim_end_matches("_").to_string();
    if sanitized_path.len() > 251 {
        return Cow::Owned(sanitized_path.get(..251).unwrap().into());
    }
    Cow::from(sanitized_path)
}

#[test]
fn test_envelope_filename() {
    assert_eq!(envelope_filename(""), "");
    assert_eq!(envelope_filename("!@#!##!@#"), "_");
    assert_eq!(envelope_filename("hello!@#!##!@#world"), "hello_world");
    assert_eq!(envelope_filename("hello!@#!##!@#world###"), "hello_world");
    let long_filename : String = (0..=256).map(|_| 'A').collect::<String>();
    assert_eq!(envelope_filename(&long_filename), "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
}
