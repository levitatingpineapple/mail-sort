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
        if mailboxes.is_empty() {
            return;
        }
        let intersection = self.mailboxes.intersection(&mailboxes);
        // Send a silent notification, if no subscribed mailboxes exist
        let priority: i8 = if intersection.count() > 0 { 0 } else { -1 };
        let text = mailboxes.into_iter().collect::<Vec<String>>().join(", ");
        println!("Notifying about: {} (priority {})", text, priority);
        let message = MessageBuilder::new(&self.user, &self.token, &text)
            .set_priority(priority)
            .build();
        // TODO: Log delivery failure
        let _ = send_pushover_request(message).await;
    }
}
