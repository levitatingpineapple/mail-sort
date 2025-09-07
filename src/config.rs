use pushover_rs::{MessageBuilder, send_pushover_request};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub imap: Imap,
    pub pushover: Pushover,
}

#[derive(Deserialize, Debug)]
pub struct Imap {
    pub server: String,
    pub port: u16,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Pushover {
    user: String,
    token: String,
    mailboxes: HashSet<String>,
}

impl Pushover {
    pub async fn notify(&self, mailboxes: HashSet<String>) {
        let mut intersection: Vec<String> = self
            .mailboxes
            .intersection(&mailboxes)
            .cloned()
            .collect::<Vec<String>>();
        if !intersection.is_empty() {
            intersection.sort_unstable();
            let text = intersection.join(", ");
            println!("Notifying about: {}", text);
            let message = MessageBuilder::new(&self.user, &self.token, &text).build();
            // TODO: Log delivery failure
            let _ = send_pushover_request(message).await;
        }
    }
}
