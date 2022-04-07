pub fn serialize<S: serde::Serializer>(
    value: &users::User,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serde::Serialize::serialize(&value.name().to_str().unwrap(), serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<users::User, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let user_name = &<String as serde::Deserialize>::deserialize(deserializer)?;
    users::get_user_by_name(user_name)
        .ok_or_else(|| serde::de::Error::custom(format!("user not found: '{}'", user_name)))
}

#[cfg(test)]
mod tests {

    use vsmtp_common::re::serde_json;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct S {
        #[serde(
            serialize_with = "crate::parser::syst_user::serialize",
            deserialize_with = "crate::parser::syst_user::deserialize"
        )]
        v: users::User,
    }

    #[test]
    fn basic() {
        assert_eq!(
            serde_json::from_str::<S>("{\"v\":\"root\"}")
                .unwrap()
                .v
                .uid(),
            users::get_user_by_name("root").unwrap().uid()
        );

        assert_eq!(
            "{\"v\":\"root\"}",
            serde_json::to_string(&S {
                v: users::get_user_by_name("root").unwrap()
            })
            .unwrap()
        );
    }
}
