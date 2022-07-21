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

/// get access to the email context.
///
/// # Return
///
/// * `the context`
///
/// # Effective smtp stage
/// all of them.
///
/// # Example
/// ```js
/// #{
///     connect: [
///        action "client ip" || log("info", `client: {ctx().client_ip}`),
///     ]
/// }
/// ```
fn ctx() { CTX }

/// get access to the server api.
fn srv() { SRV }
/// get access to the message of the email.
fn msg() { MSG }

/// check if the recipient passed as argument is part of the
/// domains (root & sni) of the server.
///
/// # Args
///
/// * `rcpt` - the recipient to check, of type string | `object address` | rcpt.
///
/// # Return
///
/// * `bool` - true of the recipient's domain is part of the server's root or sni domains, false otherwise.
///
/// # Effective smtp stage
/// all of them, but should be use in the rcpt stage.
///
/// # Example
/// ```js
/// #{
///     rcpt: [
///        rule "check rcpt domain" || if in_domain(ctx().rcpt) { next() } else { deny() },
///     ]
/// }
///
/// # Module:Utils
/// ```
fn in_domain(rcpt) { __in_domain(rcpt) }
