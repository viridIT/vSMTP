use crate::mail::Mail;

/// An abstract mail parser
pub trait MailParser: Default {
    /// Return a RFC valid [`Mail`] object
    ///
    /// # Errors
    ///
    /// * the input is not compliant
    fn parse(&mut self, bytes: &[u8]) -> anyhow::Result<Mail>;
}
