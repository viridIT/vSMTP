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
