/*
 *  envelop.rs
 *
 *  definition of envelop data structure,
 *  containing helo / ehlo, mail from and rcpt data.
 *
 *  viridit - ltabis
*/

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Envelop {
    pub helo: String,
    pub mail_from: String,
    pub recipients: Vec<String>,
}

// TODO: need error handling in case of erroneous formatting.
fn remove_inbox_characters(inbox: &str) -> String {
    inbox.trim().replace(&['<', '>'][..], "")
}

impl Envelop {
    pub fn set_sender(&mut self, sender: &str) {
        self.mail_from = remove_inbox_characters(sender);
    }

    pub fn add_rcpt(&mut self, recipient: &str) {
        self.recipients.push(remove_inbox_characters(recipient));
    }

    // TODO: need error handling (i.e. @blablah.com should return an error.)
    pub fn get_rcpt_usernames(&self) -> Vec<&str> {
        self.recipients
            .iter()
            .map(|recipient| {
                // TODO: find a way to remove everything after the '@' delimiter.
                // for now just using splitn, not really clean.
                Some(recipient.split_once("@").map_or(&**recipient, |x| x.0)).unwrap_or(recipient)
            })
            .collect()
    }
}
