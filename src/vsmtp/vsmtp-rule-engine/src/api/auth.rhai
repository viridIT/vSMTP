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

//
// standard library for authentication
//

///
/// Block relaying.
///
/// From outside : if the recipient is in the current domain or if the sender is authenticated.
///
/// From inside : if the client is in the allowed_hosts.
///
/// - args: the allowed hosts
/// - return:
///   - next()
///   - if an unauthorized relaying is detected : send a SMTP code and skip the recipient.
///
/// ### Required stage: mail+
///
/// ### Example
/// ```
/// rcpt: [
///     rule "check spf" || vsl::check_spf!()
/// ]
/// ```
///
fn check_relay(allowed_hosts) {

    let srv_domain = get_domain();

    if !(ctx().is_auth || (ctx().client_ip in allowed_hosts)) && (ctx().rcpt.domain != srv_domain) {
	delete_rcpt(ctx().rcpt);
        deny(code554_7_1)
    } else {
	next()
    }
}

/// check spf record.
/// a wrapper with the policy set to "strict" by default.
///
/// # Args
/// @identity: "helo" | "mail_from" | "both"
/// @header: "spf" | "auth" | "both" | "none"
///
fn check_spf(identity, header) {
    check_spf(identity, header, "strict")
}

/// check spf record.
/// Sender Policy Framework (RFC 7208).
///
/// # Args
/// @identity: "helo" | "mail_from" | "both"
/// @header: "spf" | "auth" | "both" | "none"
/// @policy: "strict" | "soft"
///
/// # Return
/// - deny() or next()
///
/// # Required stage : rcpt+
///
/// # Example
/// ```
/// rcpt: [
///     rule "check spf" || check_spf("both", "spf", "soft")
/// ]
/// ```
fn check_spf(identity, header, policy) {

    let query = sys::check_spf(ctx(), srv(), identity);

    if policy == "strict" {
        switch query.result {
            "pass" => {},
            "fail" => return deny(code550_7_23),
            "temperror" => return deny(code451_7_24),
            "permerror" => return deny(code550_7_24),
            _ => set_header("subject", "JUNK: " + get_header("subject")),
        }
    }

    // Record the result in a header (RFC 7208-9)
    let spf_header =
        // + if query.result == "fail" { ` ( ${query.cause} )` }
`${query.result}
 receiver=${hostname()}; client-ip=${ctx().client_ip};
 identity=mailfrom; envelope_from=${ctx().mail_from};
 ${if query.result == "pass" { `mechanism=${query.cause};` }}`;
        // + if (query.result == "temperror" || query.result == "permerror") { "\n problem=${query.cause};" };

    let auth_header =
`${hostname()}; spf=${query.result}
 reason=\"${spf_header}\" smtp.mailfrom=${ctx().mail_from}
`;

    // The Received-SPF header field is a trace field
    // and SHOULD be prepended to the existing header, above the Received: field
    // It MUST appear above all other Received-SPF fields in the message.
    switch header {
        // It is RECOMMENDED that SMTP receivers record the result"
        "none" => {},
        "spf" => add_header("Received-SPF", spf_header),
        "auth" => add_header("Authentication-Results", auth_header),
        _ => {
            add_header("Authentication-Results", auth_header);
            add_header("Received-SPF", spf_header);
        },
    }

    next()
}

///
/// Null MX sender policy (RFC 7505).
///
/// - args : no
/// - return : deny() or next()
///
/// ### Required stage : mail+
///
/// ### Example
/// ```
/// rcpt: [
///     rule "check null MX" || vsl::check_null_mx_sender!()
/// ]
/// ```
fn check_null_mx_sender() {

    let srv_domain = get_domain();

    // Check if policy is already applied or set to none
    if (srv_domain.auth.null_mx.trigger != "vsl") || (srv_domain.auth.null_mx.policy == "none") {
        return sys::next();
    }

    let query = sys::check_null_mx_sender(ctx().mail_from.domain);
    // sys::check_null_MX_sender(ctx().mail_from.domain)
    //
    // - returns : boolean (true if null MX found)

    // Null MX record not found, skip the rule.
    if !query {
        return sys::next();
    }

    // Null MX record found.
    if srv_domain.auth.null_mx.policy == "strict" {
        return sys::deny(code550_7_27);
    }

    // There's no standard header for null MX
    if srv_domain.auth.null_mx.header == "X-nullMX" {
        let header = `${hostname()}; null MX sender detected for ${ctx().mail_from.domain}`;
        sys::add_header(ctx(), "X-NullMX", header);
    }

    // Modify the subject
    sys::set_header(ctx(), "Subject", "JUNK:" + sys::get_header(ctx(), "Subject"));

    return sys::next();
}
