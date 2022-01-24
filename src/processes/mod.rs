pub mod delivery;
pub mod mime;

#[derive(Debug)]
/// used to send different types of data to vsmtp's processes.
pub struct ProcessMessage {
    pub message_id: String,
}
