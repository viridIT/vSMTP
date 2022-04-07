pub fn serialize<S: serde::Serializer>(
    value: &semver::VersionReq,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serde::Serialize::serialize(&value.to_string(), serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<semver::VersionReq, D::Error>
where
    D: serde::Deserializer<'de>,
{
    semver::VersionReq::parse(&<String as serde::Deserialize>::deserialize(deserializer)?)
        .map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use vsmtp_common::re::serde_json;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct S {
        #[serde(
            serialize_with = "crate::parser::semver::serialize",
            deserialize_with = "crate::parser::semver::deserialize"
        )]
        v: semver::VersionReq,
    }

    #[test]
    fn serialize_deserialize() {
        let str = r#"{"v":"^1.0.0"}"#;
        let s: S = serde_json::from_str(str).unwrap();
        assert_eq!(str, serde_json::to_string(&s).unwrap());
    }
}
