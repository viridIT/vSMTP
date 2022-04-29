/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
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

pub fn opt_serialize<S>(group: &Option<users::Group>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    if let Some(group) = group {
        serde::Serialize::serialize(&group.name().to_str().unwrap(), serializer)
    } else {
        serializer.serialize_none()
    }
}

pub fn opt_deserialize<'de, D>(deserializer: D) -> Result<Option<users::Group>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let group_name = &<Option<String> as serde::Deserialize>::deserialize(deserializer)?;
    if let Some(group_name) = group_name {
        Ok(Some(users::get_group_by_name(group_name).ok_or_else(
            || serde::de::Error::custom(format!("group not found: '{}'", group_name)),
        )?))
    } else {
        Ok(None)
    }
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

    #[derive(serde::Serialize, serde::Deserialize)]
    struct OptS {
        #[serde(default)]
        #[serde(
            serialize_with = "crate::parser::syst_group::opt_serialize",
            deserialize_with = "crate::parser::syst_group::opt_deserialize"
        )]
        v: Option<users::Group>,
    }

    #[test]
    fn optional() {
        assert_eq!(
            serde_json::from_str::<OptS>("{\"v\":\"root\"}")
                .unwrap()
                .v
                .unwrap()
                .gid(),
            users::get_group_by_name("root").unwrap().gid()
        );

        assert!(serde_json::from_str::<OptS>("{\"v\":null}")
            .unwrap()
            .v
            .is_none());

        assert_eq!(
            "{\"v\":\"root\"}",
            serde_json::to_string(&OptS {
                v: Some(users::get_group_by_name("root").unwrap())
            })
            .unwrap()
        );

        assert_eq!(
            "{\"v\":null}",
            serde_json::to_string(&OptS { v: None }).unwrap()
        );
    }
}
