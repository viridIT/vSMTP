///
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Deserialize,
    serde::Serialize,
    strum::EnumString,
    strum::EnumVariantNames,
    strum::EnumDiscriminants,
    strum::Display,
    strum::EnumIter,
)]
#[strum(serialize_all = "PascalCase")]
#[serde(rename_all = "PascalCase")]
#[strum_discriminants(derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub enum CodeID {
    //
    // Specials Messages
    //
    /// First message sent by the server
    Greetings,
    ///
    Help,
    ///
    Closing,
    ///
    Helo,
    ///
    EhloPain,
    ///
    EhloSecured,
    ///
    DataStart,
    //
    // SessionStatus
    //
    /// Accepted
    Ok,
    ///
    Denied,
    ///
    Failure,
    //
    // Parsing Command
    //
    ///
    UnrecognizedCommand,
    ///
    SyntaxErrorParams,
    ///
    ParameterUnimplemented,
    ///
    Unimplemented,
    ///
    BadSequence,
    //
    // TLS extension
    //
    ///
    TlsNotAvailable,
    ///
    AlreadyUnderTLS,
    /// The policy of the server require the client to be in a secured connection for a mail transaction,
    /// must use `STARTTLS`
    TlsRequired,
    //
    // Auth extension
    //
    ///
    AuthSucceeded,
    ///
    AuthMechNotSupported,
    ///
    AuthClientMustNotStart,
    ///
    AuthMechanismMustBeEncrypted,
    ///
    AuthInvalidCredentials,
    /// The policy of the server require the client to be authenticated for a mail transaction
    AuthRequired,
    ///
    AuthClientCanceled,
    ///
    AuthErrorDecode64,
    //
    // Security mechanism
    //
    /// The number of connection maximum accepted as the same time as been reached
    ConnectionMaxReached,
    /// The threshold `error_count` has been passed, then server will shutdown the connection
    TooManyError,
    ///
    Timeout,
    ///
    TooManyRecipients,
}
