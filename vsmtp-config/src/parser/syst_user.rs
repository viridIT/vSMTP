pub fn serialize<S: serde::Serializer>(
    value: &users::User,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serde::Serialize::serialize(&value.name(), serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<users::User, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let user_name = &<String as serde::Deserialize>::deserialize(deserializer)?;
    users::get_user_by_name(user_name)
        .ok_or_else(|| serde::de::Error::custom(format!("user not found: '{}'", user_name)))
}
