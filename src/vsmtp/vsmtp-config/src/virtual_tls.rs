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
use crate::{
    field::{FieldServerVirtualTls, SecretFile},
    parser::{tls_certificate, tls_private_key},
};
use vsmtp_common::re::anyhow;

impl<'de> serde::Deserialize<'de> for SecretFile<rustls::PrivateKey> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self {
            inner: tls_private_key::from_string(&s).map_err(serde::de::Error::custom)?,
            path: s.into(),
        })
    }
}

impl<'de> serde::Deserialize<'de> for SecretFile<rustls::Certificate> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self {
            inner: tls_certificate::from_string(&s).map_err(serde::de::Error::custom)?,
            path: s.into(),
        })
    }
}

impl<'de> serde::Deserialize<'de> for SecretFile<rsa::RsaPrivateKey> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self {
            inner: <rsa::RsaPrivateKey as rsa::pkcs8::DecodePrivateKey>::read_pkcs8_pem_file(&s)
                .or_else(|_| {
                    <rsa::RsaPrivateKey as rsa::pkcs1::DecodeRsaPrivateKey>::read_pkcs1_pem_file(&s)
                })
                .map_err(serde::de::Error::custom)?,
            path: s.into(),
        })
    }
}

impl FieldServerVirtualTls {
    /// create a virtual tls configuration from the certificate & private key paths.
    ///
    /// # Errors
    ///
    /// * certificate file not found.
    /// * private key file not found.
    pub fn from_path(certificate: &str, private_key: &str) -> anyhow::Result<Self> {
        Ok(Self {
            protocol_version: vec![rustls::ProtocolVersion::TLSv1_3],
            certificate: SecretFile::<rustls::Certificate> {
                inner: tls_certificate::from_string(certificate)?,
                path: certificate.into(),
            },
            private_key: SecretFile::<rustls::PrivateKey> {
                inner: tls_private_key::from_string(private_key)?,
                path: private_key.into(),
            },
            sender_security_level: Self::default_sender_security_level(),
        })
    }
}
