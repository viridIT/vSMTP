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

/// check "mail from" relaying.
/// Do not accept a msg from a known internal domain if the client is unknown 
///
/// # Args
/// @allowed_hosts: group of (IPv4 | IPv6 | IPv4 range | IPv6 range | fqdn)
///
/// # Return
/// - deny() or next()
///
/// # Required stage : mail (only)
///
/// # Example 
/// ```
/// mail: [
///    rule "check mail relay" || {
///        object allowed_hosts group = [
///            object ip4 "192.168.1.254",
///            object fqdn "mta-internal.foobar.com"
///        ];
///        check_mail_relay(allowed_hosts);
///    }        
/// ]
fn check_mail_relay(allowed_hosts) {
    if in_domain(ctx().mail_from)
    && !(ctx().is_authenticated || (ctx().client_ip in allowed_hosts)) {
        deny(code554_7_1)
    } else {
        next()
    }    
}

/// check "rcpt to" relaying.
/// Do not accept open relaying
///
/// # Args
/// @allowed_hosts: group of (IPv4 | IPv6 | IPv4 range | IPv6 range | fqdn)
///
/// # Return
/// - deny() or next()
///
/// # Required stage : rcpt (only)
///
/// # Example 
/// ```
/// rcpt: [
///    rule "check rcpt relay" || {
///        object allowed_hosts group = [
///            object ip4 "192.168.1.254",
///            object fqdn "mta-internal.foobar.com"
///        ];
///        check_rcpt_relay(allowed_hosts);
///    }        
/// ]
fn check_rcpt_relay(allowed_hosts) {
    if !in_domain(ctx().rcpt)
    && !(ctx().is_authenticated || (ctx().client_ip in allowed_hosts)) {
        remove_rcpt(ctx().rcpt.to_string());
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

/// create key-value pairs of spf results
/// to inject into the spf or auth headers.
private fn spf_key_value_list(query) {
`receiver=${hostname()}; client-ip=${ctx().client_ip};
 identity=mailfrom; envelope_from=${ctx().mail_from};
 ${
   if "mechanism" in query { `mechanism=${query.mechanism};` }
   else if "problem" in query { `problem=${query.problem};` }
 }`
}

/// Record results in a spf header (RFC 7208-9)
private fn spf_header(query) {
    `${query.result} ${spf_key_value_list(query)}`
}

/// Record results in the auth header (RFC 7208-9)
fn auth_header(query) {
`${hostname()}; spf=${query.result}
 reason="${spf_key_value_list(query)}"
 smtp.mailfrom=${ctx().mail_from}`;
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

    const AUTH_HEADER = "Authentication-Results";
    const SPF_HEADER = "Received-SPF";

    let query = sys::check_spf(ctx(), srv(), identity);

    // TODO: The Received-SPF header field is a trace field
    // and SHOULD be prepended to the existing header, above the Received: field
    // It MUST appear above all other Received-SPF fields in the message.
    switch header {
        // It is RECOMMENDED that SMTP receivers record the result"
        "spf" => prepend_header(SPF_HEADER, spf_header(query)),
        "auth" => prepend_header(AUTH_HEADER, auth_header(query)),
        "both" => {
            prepend_header(AUTH_HEADER, auth_header(query));
            prepend_header(SPF_HEADER, spf_header(query));
        },
        _ => {},
    }

    if policy == "strict" {
        switch query.result {
            "pass" => next(),
            "fail" => deny(code550_7_23),
            "softfail" => deny(code550_7_23),
            "temperror" => deny(code451_7_24),
            "permerror" => deny(code550_7_24),
            _ => next(),
        }
    } else if policy == "soft" {
        switch query.result {
            "fail" => deny(code550_7_23),
            "temperror" => deny(code451_7_24),
            "permerror" => deny(code550_7_24),
            _ => next(),
        }
    } else {
        throw `spf 'policy' argument must be 'strict' or 'soft', not '${policy}'`;
    }
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
        sys::prepend_header(ctx(), "X-NullMX", header);
    }

    // Modify the subject
    sys::set_header(ctx(), "Subject", "JUNK:" + sys::get_header(ctx(), "Subject"));

    return sys::next();
}
