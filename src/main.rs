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

use clap::{Subcommand, Parser};
use mailbox::stream::entry::Header;
use mailbox::stream::Entry;
use regex::Regex;

use filter::Filter;
use mail::{Context, Mail};

#[derive(Parser)]
#[clap(version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Count {
        file: String,
        #[clap(parse(try_from_str))]
        filter: Filter,
    },
    Extract {
        file: String,
        #[clap(parse(try_from_str))]
        filter: Filter,
    },
}

fn main() {
    match &Cli::parse().command {
        Commands::Count { file, filter } => {
            if let Err(e) = count(file, &filter) {
                eprintln!("{:?}", e);
            }
        },
        Commands::Extract { file, filter } => {
            if let Err(e) = extract(file, &filter) {
                eprintln!("{:?}", e);
            }
        }
    }
}

fn iterate(
    path: &str,
    filter: &Filter,
    mut process: impl FnMut(&Mail),
) -> Result<(), std::io::Error> {
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
                if let Some(ref mut m) = ctx.end() {
                    if filter.matches(m) {
                        process(m);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn count(path: &str, filter: &Filter) -> Result<(), std::io::Error> {
    let mut count = 0;
    iterate(path, filter, |_| {
        count += 1;
    })?;
    eprintln!("Matching entries: {}", count);
    Ok(())
}

fn extract(path: &str, filter: &Filter) -> Result<(), std::io::Error> {
    iterate(path, filter, |m| {
        let date = m.date();
        let subject = m.subject();
        let base_name = format!("{}-{}", date, subject);
        let name = envelope_filename(&base_name);
        let path = format!("{}.txt", &name);
        eprintln!("Saving email to {}", path);
        let mut file = File::create(&path).unwrap();
        let body_text = m.body_text();
        file.write_all(&body_text.into_bytes()).unwrap();
    })?;

    Ok(())
}

fn envelope_filename(path: &str) -> Cow<str> {
    let filename_regex = Regex::new(r"[^A-Za-z0-9]+").unwrap();
    let sanitized_path = filename_regex
        .replace_all(path, "_")
        .trim_end_matches('_')
        .to_string();
    if sanitized_path.len() > 251 {
        return Cow::Owned(sanitized_path.get(..251).unwrap().into());
    }
    Cow::from(sanitized_path)
}

#[test]
fn test_envelope_filename() {
    assert_eq!(envelope_filename(""), "");
    assert_eq!(envelope_filename("!@#!##!@#"), "");
    assert_eq!(envelope_filename("hello!@#!##!@#world"), "hello_world");
    assert_eq!(envelope_filename("hello!@#!##!@#world###"), "hello_world");
    let long_filename: String = (0..=256).map(|_| 'A').collect::<String>();
    assert_eq!(envelope_filename(&long_filename), "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
}
