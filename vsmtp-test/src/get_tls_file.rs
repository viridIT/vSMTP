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
#[must_use]
pub const fn get_certificate() -> &'static str {
    include_str!("./template/certs/certificate.crt")
}

///
#[must_use]
pub const fn get_rsa_key() -> &'static str {
    include_str!("./template/certs/private_key.rsa.key")
}

///
#[must_use]
pub const fn get_pkcs8_key() -> &'static str {
    include_str!("./template/certs/private_key.pkcs8.key")
}

///
#[must_use]
pub const fn get_ec256_key() -> &'static str {
    include_str!("./template/certs/private_key.ec256.key")
}
