pub fn serialize<S: serde::Serializer>(
    value: &users::Group,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serde::Serialize::serialize(&value.name().to_str().unwrap(), serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<users::Group, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let group_name = &<String as serde::Deserialize>::deserialize(deserializer)?;
    users::get_group_by_name(group_name)
        .ok_or_else(|| serde::de::Error::custom(format!("group not found: '{}'", group_name)))
}

#[cfg(test)]
mod tests {

    use vsmtp_common::re::serde_json;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct S {
        #[serde(
            serialize_with = "crate::parser::syst_group::serialize",
            deserialize_with = "crate::parser::syst_group::deserialize"
        )]
        v: users::Group,
    }

    #[test]
    fn basic() {
        assert_eq!(
            serde_json::from_str::<S>("{\"v\":\"root\"}")
                .unwrap()
                .v
                .gid(),
            users::get_group_by_name("root").unwrap().gid()
        );

        assert_eq!(
            "{\"v\":\"root\"}",
            serde_json::to_string(&S {
                v: users::get_group_by_name("root").unwrap()
            })
            .unwrap()
        );
    }
}
