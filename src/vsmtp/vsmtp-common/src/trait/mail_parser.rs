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
use crate::{Either, Mail, RawBody};

///
pub type ParserOutcome = anyhow::Result<Either<RawBody, Mail>>;

/// An abstract mail parser
pub trait MailParser: Default {
    /// From a buffer of strings, return either:
    ///
    /// * a RFC valid [`Mail`] object
    /// * a [`RawBody`] instance
    ///
    /// # Errors
    ///
    /// * the input is not compliant
    fn parse_lines(&mut self, raw: &[&str]) -> ParserOutcome;

    ///
    /// # Errors
    ///
    /// * the input is not compliant
    fn parse_raw(&mut self, raw: &RawBody) -> ParserOutcome {
        let headers = raw
            .headers_lines()
            .into_iter()
            .chain(std::iter::once("\r\n"));

        self.parse_lines(
            &if let Some(body) = raw.body_lines() {
                headers.chain(body).collect::<Vec<_>>()
            } else {
                headers.collect::<Vec<_>>()
            }[..],
        )
    }
}

/// An abstract async mail parser
#[allow(clippy::module_name_repetitions)]
#[async_trait::async_trait]
pub trait MailParserOnFly: Default {
    /// From a buffer of strings, return either:
    ///
    /// * a RFC valid [`Mail`] object
    /// * a [`RawBody`] instance
    ///
    /// # Errors
    ///
    /// * the input is not compliant
    async fn parse<'a>(
        &'a mut self,
        stream: impl tokio_stream::Stream<Item = String> + Unpin + Send + 'a,
    ) -> ParserOutcome;
}

// #[async_trait::async_trait]
// impl<T> MailParserOnFly for T
// where
//     T: MailParser + Send + Sync,
// {
//     async fn parse<'a>(
//         &'a mut self,
//         mut stream: impl tokio_stream::Stream<Item = String> + Unpin + Send + 'a,
//     ) -> anyhow::Result<MessageBody> {
//         let mut buffer = vec![];
//         while let Some(i) = tokio_stream::StreamExt::next(&mut stream).await {
//             buffer.push(i);
//         }
//         self.parse(buffer)
//     }
// }
