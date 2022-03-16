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
