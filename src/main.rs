use clap::Parser;
use config::Config;
use imap::{
    self, ClientBuilder, Session, extensions::idle::WaitOutcome, types::UnsolicitedResponse,
};
use mailparse::{self, MailAddr, addrparse_header, parse_header};
use std::{
    collections::{HashMap, HashSet},
    fs::read_to_string,
    io::{self, Read, Write},
    path::PathBuf,
};

mod config;

#[derive(Parser)]
#[command(about = "Sort emails into mailboxes based on recipient addresses")]
struct Args {
    #[arg(long, help = "Path to the config file")]
    config: PathBuf,
}

type Sorted = HashMap<String, HashSet<u32>>;

#[tokio::main]
async fn main() -> Result<(), Err> {
    let args = Args::parse();
    let config: Config = toml::from_str(&read_to_string(args.config)?)?;
    let mut session = ClientBuilder::new(&config.imap.server, config.imap.port)
        .connect()?
        .login(&config.imap.email, &config.imap.password)
        .map_err(|e| e.0)?;
    session.select("INBOX")?;

    // Do initial mail sort
    sort_mail(&mut session, &config.pushover)?;

    loop {
        // Idle and wait for `Exists` messages which indicate mail count change
        let result = session.idle().wait_while(|response| match response {
            UnsolicitedResponse::Exists(_) => false,
            _ => {
                dbg!(response);
                true
            }
        });
        // Sort mail if mailbox has changed
        match result {
            Ok(wait_outcome) => match wait_outcome {
                WaitOutcome::TimedOut => {
                    println!("Timed Out");
                    continue;
                }
                WaitOutcome::MailboxChanged => {
                    println!("Mailbox Changed");
                    sort_mail(&mut session, &config.pushover)?
                }
            },
            Result::Err(error) => {
                dbg!(error);
                break;
            }
        }
    }
    session.logout().expect("logout");
    Ok(())
}

/// Moves emails and creates mailboxes if required
fn sort_mail<T: Write + Read>(
    session: &mut Session<T>,
    pushover: &config::Pushover,
) -> Result<(), Err> {
    let existing = mailboxes(session)?;
    let sorted = sort_inbox(session)?;
    for (mailbox, ids) in sorted.iter() {
        if !existing.contains(mailbox) {
            session.create(mailbox)?;
            session.subscribe(mailbox)?;
            println!("Created {}", mailbox);
        }
        let ids_string = ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<String>>()
            .join(",");
        session.uid_mv(&ids_string, mailbox)?;
        println!("Moved {}, to: {}", &ids_string, &mailbox);
    }
    let pushover = pushover.clone();
    tokio::spawn(async move {
        pushover.notify(sorted.into_keys().collect()).await;
    });
    Ok(())
}

/// Returns names of all existing mailboxes
fn mailboxes<T: Write + Read>(session: &mut Session<T>) -> Result<HashSet<String>, Err> {
    Ok(session
        .list(Some(""), Some("*"))?
        .iter()
        .map(|n| n.name().to_string())
        .collect())
}

/// Returns messages sorted by (single!) `To` address.
fn sort_inbox<T: Write + Read>(session: &mut Session<T>) -> Result<Sorted, Err> {
    let fetches = session.uid_fetch("1:*", "BODY.PEEK[HEADER.FIELDS (TO)]")?;
    let mut sorted = Sorted::new();
    for fetch in fetches.iter() {
        let header_data = fetch.header().ok_or(Err::MissingHeader)?;
        let (mail_header, _) = parse_header(header_data)?;
        let address_list = addrparse_header(&mail_header)?;
        let uid = fetch.uid.ok_or(Err::MissingUid)?;
        if let Some(address) = address_list.first() {
            match address {
                MailAddr::Group(_) => { /* Move groups manually */ }
                MailAddr::Single(single) => {
                    let mailbox = mailbox_from(&single.addr);
                    let uids = sorted.entry(mailbox).or_insert(HashSet::default());
                    uids.insert(uid);
                }
            }
        }
    }
    Ok(sorted)
}

/// Converts email address in to a mailbox name
fn mailbox_from(address: &str) -> String {
    let mut string = String::new();
    let mut parts = address.splitn(2, "@");
    let username = parts.next().expect("First part");
    if let Some(domain) = parts.next() {
        push_no_dots(domain, &mut string);
    }
    string.push('.');
    push_no_dots(username, &mut string);
    string.to_lowercase()
}

fn push_no_dots(input: &str, base: &mut String) {
    for char in input.chars() {
        if char == '.' {
            base.push('_');
        } else {
            base.push(char);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Err {
    #[error("Missing Header")]
    MissingHeader,
    #[error("Missing UID")]
    MissingUid,
    #[error("Imap: {0}")]
    Imap(#[from] imap::error::Error),
    #[error("Parse: {0}")]
    Parse(#[from] mailparse::MailParseError),
    #[error("IO Error {0}")]
    IO(#[from] io::Error),
    #[error("Toml error")]
    Toml(#[from] toml::de::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_to_mailbox() {
        assert_eq!(
            "example_com.auth_service",
            &mailbox_from("auth.service@example.com")
        );
    }
}
