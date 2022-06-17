/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
use crate::rcpt::Rcpt;
use crate::Address;

/// Data receive during a smtp transaction
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Envelop {
    /// result of the HELO/HELO command.
    pub helo: String,
    /// the sender of the email received using the MAIL FROM command.
    pub mail_from: Address,
    /// a list of recipients received using the RCPT TO command.
    pub rcpt: Vec<Rcpt>,
}

impl Default for Envelop {
    fn default() -> Self {
        Self {
            helo: String::default(),
            mail_from: Address::new_unchecked("default@domain.com".to_string()),
            rcpt: vec![],
        }
    }
}

/// build a [lettre] envelop using from address & recipients.
///
/// # Errors
/// * Could not create lettre address.
// fn build_lettre<Iter>(
//     from: &Address,
//     rcpt: impl IntoIterator<Item = impl AsRef<Rcpt>>,
// ) -> anyhow::Result<lettre::address::Envelope> {
//     lettre::address::Envelope::new(
//         Some(
//             from.full()
//                 .parse()
//                 .context("failed to parse `from` address")?,
//         ),
//         rcpt.into_iter()
//             .map(|rcpt| {
//                 rcpt.as_ref()
//                     .address
//                     .full()
//                     .parse::<lettre::Address>()
//                     .context("failed to parse `to` address")
//             })
//             .collect::<anyhow::Result<Vec<_>>>()?,
//     )
//     .context("failed to build the envelop")
// }

#[cfg(test)]
pub mod test {
    use crate::mail_context::ConnectionContext;

    #[allow(clippy::missing_panics_doc)]
    #[must_use]
    /// create an empty email context for testing purposes.
    pub fn get_default_context() -> crate::mail_context::MailContext {
        crate::mail_context::MailContext {
            connection: ConnectionContext {
                timestamp: std::time::SystemTime::now(),
                credentials: None,
                is_authenticated: false,
                is_secured: false,
                server_name: "testserver.com".to_string(),
                server_address: "0.0.0.0:25".parse().unwrap(),
            },
            client_addr: "0.0.0.0:0".parse().unwrap(),
            envelop: crate::envelop::Envelop::default(),
            metadata: Some(crate::mail_context::MessageMetadata {
                timestamp: std::time::SystemTime::now(),
                ..crate::mail_context::MessageMetadata::default()
            }),
        }
    }

    // #[test]
    // fn test_build_lettre_envelop() {
    //     assert_eq!(
    //         build_lettre(
    //             &addr!("a@a.a"),
    //             &[Rcpt {
    //                 address: addr!("b@b.b"),
    //                 transfer_method: Transfer::None,
    //                 email_status: EmailTransferStatus::Sent
    //             }]
    //         )
    //         .expect("failed to build lettre envelop"),
    //         lettre::address::Envelope::new(
    //             Some("a@a.a".parse().unwrap()),
    //             vec!["b@b.b".parse().unwrap()]
    //         )
    //         .unwrap()
    //     );
    // }
}
