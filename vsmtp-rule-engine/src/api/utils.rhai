/// get the domain used to identify a recipient.
private fn __in_domain(rcpt) {
    switch rcpt.type_of() {
        "vsmtp_common::address::Address" => {
            rcpt.domain in toml::server.virtual || rcpt.domain == toml::server.domain
        },
        "string" => {
            // converting to an address could fail.
            try { return __in_domain(vsl::new_address(rcpt)); } catch { return false; }
        },
        _ => false,
    }
}
