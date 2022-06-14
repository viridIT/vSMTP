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
/// The credentials send by the client, not necessarily the right one
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize, strum::Display)]
#[strum(serialize_all = "PascalCase")]
pub enum Credentials {
    /// the pair will be sent and verified by a third party
    Verify {
        ///
        authid: String,
        ///
        authpass: String,
    },
    /// the server will query a third party and make internal verification
    Query {
        ///
        authid: String,
    },
    /// verify the token send by anonymous mechanism
    AnonymousToken {
        /// [ email / 1*255TCHAR ]
        token: String,
    },
}
