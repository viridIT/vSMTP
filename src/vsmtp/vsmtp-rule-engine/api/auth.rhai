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

/// Do not accept a message from a known internal domain if the client is unknown.
///
/// # Args
/// * `allowed_hosts` - group of IPv4 | IPv6 | IPv4 range | IPv6 range | fqdn
///
/// # Return
/// * `deny()`
/// * `next()`
///
/// # Effective smtp stage
/// `mail` and onwards.
///
/// # Example
/// ```js
/// mail: [
///    rule "check mail relay" || {
///        object allowed_hosts group = [
///            object mta_ip ip4 = "192.168.1.254",
///            object mta_fqdn fqdn = "mta-internal.foobar.com"
///        ];
///        check_mail_relay(allowed_hosts)
///    }
/// ]
///
/// # Module:Security
/// ```
fn check_mail_relay(allowed_hosts) {
    if in_domain(ctx().mail_from)
    && !(ctx().is_authenticated || (ctx().client_ip in allowed_hosts)) {
        deny(code554_7_1)
    } else {
        next()
    }
}

/// Do not accept open relaying.
///
/// # Args
///
/// * `allowed_hosts` - group of IPv4 | IPv6 | IPv4 range | IPv6 range | fqdn
///
/// # Return
/// * `deny()`
/// * `next()`
///
/// # Effective smtp stage
/// `rcpt` only.
///
/// # Example
/// ```js
/// rcpt: [
///    rule "check rcpt relay" || {
///        object allowed_hosts group = [
///            object mta_ip ip4 = "192.168.1.254",
///            object mta_fqdn fqdn = "mta-internal.foobar.com"
///        ];
///        check_rcpt_relay(allowed_hosts)
///    }
/// ]
///
/// # Module:Security
/// ```
fn check_rcpt_relay(allowed_hosts) {
    if !in_domain(ctx().rcpt)
    && !(ctx().is_authenticated || (ctx().client_ip in allowed_hosts)) {
        remove_rcpt_envelop(ctx().rcpt);
        info(code554_7_1)
    } else {
        next()
    }
}

/// create key-value pairs of spf results
/// to inject into the spf or auth headers.
private fn spf_key_value_list(query) {
`receiver=${hostname()};
 client-ip=${ctx().client_ip};
 envelope_from=${ctx().mail_from};
 identity=mailfrom;
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


/// Check spf record following the Sender Policy Framework (RFC 7208).
/// see https://datatracker.ietf.org/doc/html/rfc7208
///
/// # Args
///
/// * `header` - "spf" | "auth" | "both" | "none"
/// * `policy` - "strict" | "soft"
///
/// # Return
/// * `deny(code550_7_23 | code451_7_24 | code550_7_24)` - an error occured during lookup. (returned even when a softfail is received using the "strict" policy)
/// * `next()` - the operation succeded.
///
/// # Errors
/// * The `header` argument is not valid.
/// * The `policy` argument is not valid.
///
/// # Effective smtp stage
/// `rcpt` and onwards.
///
/// # Example
/// ```js
/// #{
///     rcpt: [
///        rule "check spf" || check_spf("spf", "soft")
///     ]
/// }
///
/// # Module:Security
/// ```
fn check_spf(header, policy) {

    if in_domain(ctx().mail_from) {
        return next();
    }

    const AUTH_HEADER = "Authentication-Results";
    const SPF_HEADER = "Received-SPF";

    let query = sys::check_spf(ctx(), srv());

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
        "none" => {},
        _ => throw `spf 'header' argument must be 'spf', 'auth' or 'both', not '${header}'`,
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

/// Check spf record following the Sender Policy Framework (RFC 7208).
/// A wrapper with the policy set to "strict" by default.
/// see https://datatracker.ietf.org/doc/html/rfc7208
///
/// # Args
///
/// * `header` - "spf" | "auth" | "both" | "none"
///
/// # Return
/// * `deny(code550_7_23 | code451_7_24 | code550_7_24)` - an error occured during lookup. (returned even when a softfail is received using the "strict" policy)
/// * `next()` - the operation succeded.
///
/// # Effective smtp stage
/// `rcpt` and onwards.
///
/// # Errors
/// * The `header` argument is not valid.
/// * The `policy` argument is not valid.
///
/// # Example
/// ```js
/// #{
///     rcpt: [
///        rule "check spf relay" || check_spf(allowed_hosts),
///     ]
/// }
///
/// # Module:Security
/// ```
fn check_spf(header) {
    check_spf(header, "strict")
}

private fn sys_dkim_verify(policy) {
    log("warn", `verifying DKIM signature with policy=${policy}`);

    let last_error = `dkim=none`;

    for i in msg().headers("DKIM-Signature", policy.nbr_headers) {
        try {
            let signature = sys::parse_signature(i);

            if signature.has_expired(policy.expiration_epsilon) {
                log("warn", `DKIM signature expired, continuing`);
                continue;
            }

            // NOTE: for any reason, you can decide to ignore the signature
            // if signature... {
            //     continue;
            // }

            for key in sys::get_public_key(srv(), signature, policy.on_multiple_key_records) {
                try {
                    sys::dkim_verify(msg(), signature, key);
                    log("warn", `DKIM signature successfully verified`);

                    if key.has_debug_flag {
                        log("warn", `DKIM signature contains debug_flag, continuing`);
                        continue;
                    }

                    // header.b & header.a can be set optionally
                    return `dkim=pass header.d=${signature.sdid} header.i=${signature.auid};`;
                } catch (e) {
                    log("warn", `DKIM signature verifification failed: ${e}`);
                    let e = sys::handle_dkim_error(e);
                    last_error = `dkim=${e}`;
                }
            }

        } catch (e) {
            log("warn", `DKIM signature invalid: ${e}`);
            let e = sys::handle_dkim_error(e);
            last_error = `dkim=${e}`;
        }
    }

    log("warn", "no valid DKIM signature, returning");
    last_error
}

/// Verify the `DKIM-Signature` header(s) in the mail and produce a `Authentication-Results`.
/// see https://datatracker.ietf.org/doc/html/rfc6376
///
/// # Return
/// * `accept()` - a signature was successfuly verified.
/// * `deny()` - no signature could be verified.
///
/// # Module:Security
fn dkim_verify() {
    let result = sys_dkim_verify(#{
        // will process a limited amount of signature, avoid infinite loop and dos/ddos attack
        nbr_headers: 5,
        // the dns query may result in multiple public key, the registry with invalid format are ignored.
        // among ["first_one", "cycle"]
        on_multiple_key_records: "cycle",
        // is the `expire_time` of the signature over `now +/- eplison` (as seconds)
        expiration_epsilon: 100,
    });

    let header = `${hostname()};
  ${result}`;

    prepend_header("Authentication-Results", header);
    result
}

/// Produce a `DKIM-Signature` header.
fn dkim_sign(selector, headers_field) {
    sys::dkim_sign(msg(), ctx(), srv(), selector, headers_field)
}
