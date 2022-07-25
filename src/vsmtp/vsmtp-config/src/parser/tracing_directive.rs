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
pub fn serialize<S: serde::Serializer>(
    value: &Vec<tracing_subscriber::filter::Directive>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut x = serializer.serialize_seq(Some(value.len()))?;
    for i in value {
        serde::ser::SerializeSeq::serialize_element(&mut x, &i.to_string())?;
    }
    serde::ser::SerializeSeq::end(x)
}

pub fn deserialize<'de, D>(
    deserializer: D,
) -> Result<Vec<tracing_subscriber::filter::Directive>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = <Vec<String> as serde::Deserialize>::deserialize(deserializer)?;

    value
        .into_iter()
        .map(|s| <tracing_subscriber::filter::Directive as std::str::FromStr>::from_str(&s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| serde::de::Error::custom(format!("invalid directive: `{e}`")))
}
