use crate::server_config::{ProtocolVersion, ProtocolVersionRequirement};

#[test]
fn one_string() {
    #[derive(Debug, serde::Deserialize)]
    struct S {
        v: ProtocolVersionRequirement,
    }

    let s = toml::from_str::<S>(r#"v = "TLSv1.2""#).unwrap();
    assert_eq!(
        s.v.0,
        vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_2)]
    );
    let s = toml::from_str::<S>(r#"v = "0x0303""#).unwrap();
    assert_eq!(
        s.v.0,
        vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_2)]
    );

    let s = toml::from_str::<S>(r#"v = "TLSv1.3""#).unwrap();
    assert_eq!(
        s.v.0,
        vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_3)]
    );
    let s = toml::from_str::<S>(r#"v = "0x0304""#).unwrap();
    assert_eq!(
        s.v.0,
        vec![ProtocolVersion(rustls::ProtocolVersion::TLSv1_3)]
    );
}

#[test]
fn array() {
    #[derive(Debug, serde::Deserialize)]
    struct S {
        v: ProtocolVersionRequirement,
    }

    let s = toml::from_str::<S>(r#"v = ["TLSv1.2", "TLSv1.3"]"#).unwrap();
    assert_eq!(
        s.v.0,
        vec![
            ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
            ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
        ]
    );
}

#[test]
fn pattern() {
    #[derive(Debug, serde::Deserialize)]
    struct S {
        v: ProtocolVersionRequirement,
    }

    let s = toml::from_str::<S>(r#"v = "^TLSv1.2""#).unwrap();
    assert_eq!(
        s.v.0,
        vec![
            ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
            ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
        ]
    );

    let s = toml::from_str::<S>(r#"v = ">=TLSv1.2""#).unwrap();
    assert_eq!(
        s.v.0,
        vec![
            ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
            ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
        ]
    );
}
