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

///
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, PartialEq, Eq, Clone, strum::EnumString, strum::Display)]
pub enum SigningAlgorithm {
    ///
    #[strum(serialize = "rsa-sha1")]
    RsaSha1,
    ///
    #[strum(serialize = "rsa-sha256")]
    RsaSha256,
}

///
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, PartialEq, Eq, Clone, strum::EnumIter, strum::EnumString, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum HashAlgorithm {
    ///
    Sha1,
    ///
    Sha256,
}
