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
