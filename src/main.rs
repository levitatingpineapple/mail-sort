use clap::Parser;
use imap::{self, ClientBuilder, Session};
use mailparse::{self, MailAddr};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
};

#[derive(Parser)]
#[command(about = "Sort emails into mailboxes based on recipient addresses")]
struct Args {
    #[arg(long, help = "IMAP server address")]
    server: String,

    #[arg(long, default_value_t = 993, help = "IMAP server port")]
    port: u16,

    #[arg(long, help = "Email address")]
    email: String,

    #[arg(long, help = "Password")]
    password: String,
}

fn main() -> Result<(), Err> {
    let args = Args::parse();

    let mut session = ClientBuilder::new(&args.server, args.port)
        .connect()?
        .login(&args.email, &args.password)
        .map_err(|e| e.0)?;
    if let Some(err) = sort_mail(&mut session).err() {
        println!("{}", err);
    }
    session.logout().expect("logout");
    Ok(())
}

fn sort_mail<T: Write + Read>(session: &mut Session<T>) -> Result<(), Err> {
    let mailboxes = mailboxes(session)?;
    for (mailbox, ids) in inbox_message_map(session)? {
        if !mailboxes.contains(&mailbox) {
            session.create(&mailbox)?;
            println!("Created {}", mailbox);
        }
        let uid_set = ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<String>>()
            .join(",");
        session.uid_mv(&uid_set, &mailbox)?;
        println!("Moved {}, to: {}", &uid_set, &mailbox);
    }
    Ok(())
}

// Returns names of all existing mailboxes
fn mailboxes<T: Write + Read>(session: &mut Session<T>) -> Result<HashSet<String>, Err> {
    Ok(session
        .list(Some(""), Some("*"))?
        .iter()
        .map(|n| n.name().to_string())
        .collect())
}

type MessageMap = HashMap<String, HashSet<u32>>;

// Returns messages sorted by (single!) `To` address.
fn inbox_message_map<T: Write + Read>(session: &mut Session<T>) -> Result<MessageMap, Err> {
    session.select("INBOX")?;
    let messages = session.uid_fetch("1:*", "BODY.PEEK[HEADER.FIELDS (TO)]")?;
    let mut folders = MessageMap::new();
    for message in messages.iter() {
        let header_data = message.header().ok_or(Err::MissingHeader)?;
        let (header, _) = mailparse::parse_header(header_data)?;
        let address_list = mailparse::addrparse_header(&header)?;
        let uid = message.uid.ok_or(Err::MissingUid)?;
        if let Some(address) = address_list.first() {
            match address {
                MailAddr::Group(_) => { /* Move groups manually */ }
                MailAddr::Single(single) => {
                    let addr = mailbox_name_from(&single.addr);
                    let mail_ids = folders.entry(addr).or_insert(HashSet::default());
                    mail_ids.insert(uid);
                }
            }
        }
    }
    Ok(folders)
}

/// Converts email address in to a mailbox name
fn mailbox_name_from(address: &str) -> String {
    let mut string = String::new();
    let mut parts = address.splitn(2, "@");
    let username = parts.next().expect("First part");
    if let Some(domain) = parts.next() {
        string.push_str(domain);
    }
    string.push_str("/");
    string.push_str(username);
    string.to_lowercase()
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_to_mailbox() {
        assert_eq!(
            "example.com/auth.service",
            &mailbox_name_from("auth.service@example.com")
        );
    }
}
