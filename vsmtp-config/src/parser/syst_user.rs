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
