/// used to send different types of data to vsmtp's processes.
#[derive(Debug)]
pub struct ProcessMessage {
    /// id of the mail context
    pub message_id: String,
}
