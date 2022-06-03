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
use crate::mail_context::MessageBody;

/// An abstract mail parser
pub trait MailParser: Default {
    /// Return a RFC valid [`MessageBody`] object from a buffer of strings
    ///
    /// # Errors
    ///
    /// * the input is not compliant
    fn parse(&mut self, raw: Vec<String>) -> anyhow::Result<MessageBody>;
}

/// An abstract async mail parser
#[allow(clippy::module_name_repetitions)]
#[async_trait::async_trait]
pub trait MailParserOnFly: Default {
    /// Return a RFC valid [`MessageBody`] object from a stream of strings
    ///
    /// # Errors
    ///
    /// * the input is not compliant
    async fn parse<'a>(
        &'a mut self,
        stream: impl tokio_stream::Stream<Item = String> + Unpin + Send + 'a,
    ) -> anyhow::Result<MessageBody>;
}

#[async_trait::async_trait]
impl<T> MailParserOnFly for T
where
    T: MailParser + Send + Sync,
{
    async fn parse<'a>(
        &'a mut self,
        mut stream: impl tokio_stream::Stream<Item = String> + Unpin + Send + 'a,
    ) -> anyhow::Result<MessageBody> {
        let mut buffer = vec![];
        while let Some(i) = tokio_stream::StreamExt::next(&mut stream).await {
            buffer.push(i);
        }
        self.parse(buffer)
    }
}
