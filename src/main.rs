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
use mail::Mail;

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
            eprintln!("extracting bodies from {}", path);

            let program = match command.value_of("filter") {
                Some(filter) => filter::parse(filter).unwrap().1,
                _ => Filter { expression: None },
            };

            println!("Using program to filter mail: {:?}", program);

            if let Err(e) = extract(path, &program) {
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
    let mut file: Option<File> = None;
    let mut mail: Option<Mail> = None;

    for entry in mailbox::stream::entries(File::open(path)?) {
        match entry {
            Ok(Entry::Begin(_, _)) => {
                mail = Some(Mail::new());
            },
            Ok(Entry::Header(ref header)) if filter.includes_header(header) => {
                println!("Header matched {:?}", header);
                if let Some(ref mut m) = mail {
                    m.headers.push(header.clone());
                    println!("Envelope matched {:?}", m);
                }
            }
            Ok(Entry::Body(body)) => {
                if let Some(ref mut f) = file {
                    f.write_all(&body)?;
                    f.write_all(b"\n")?;
                }
            }
            Ok(Entry::End) => {
                if let Some(ref mut m) = mail {
                    if filter.matches(m) {
                        println!("Envelope matched {:?}", m);
                    }
                }

                //if let Some(ref mut f) = file {
                    ////println!("{:?}", header.value());

                    ////let subject = &header.value();
                    ////let name = envelope_filename(subject);
                    ////let path = format!("{}.txt", &name);
                    ////file = Some(File::create(&path)?);
                    ////
                    //f.write_all(b"\n")?;
                    //f.sync_all()?;
                    //file = None;
                    //std::process::exit(0);
                //}
            }
            _ => {}
        }
    }

    Ok(())
}

fn envelope_filename<'a>(path: &'a str) -> Cow<'a, str> {
    let filename_regex = Regex::new(r"[^A-Za-z0-9]+").unwrap();
    filename_regex.replace_all(path, "_")
}