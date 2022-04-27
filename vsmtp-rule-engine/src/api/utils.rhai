/// get the domain used to identify a recipient.
private fn __get_domain(rcpt) {
    rcpt.domain in toml::server.virtual || rcpt.domain == toml::server.domain
}
