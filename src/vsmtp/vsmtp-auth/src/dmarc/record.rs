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

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("missing required field: `{field}`")]
    MissingRequiredField { field: String },
    #[error("syntax error: `{reason}`")]
    SyntaxError { reason: String },
    #[error("invalid argument: `{reason}`")]
    InvalidArgument { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, strum::EnumString, strum::Display)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Version {
    Dmarc1,
}

#[derive(Debug, strum::EnumString, strum::Display)]
enum AlignmentMode {
    #[strum(serialize = "r")]
    Relaxed,
    #[strum(serialize = "s")]
    Strict,
}

#[derive(Debug, strum::EnumString, strum::Display)]
enum FailureReportOption {
    #[strum(serialize = "0")]
    All,
    #[strum(serialize = "1")]
    Any,
    #[strum(serialize = "d")]
    Dkim,
    #[strum(serialize = "s")]
    Spf,
}

#[derive(Debug, strum::EnumString, strum::Display)]
#[strum(serialize_all = "lowercase")]
enum ReceiverPolicy {
    None,
    Quarantine,
    Reject,
}

#[derive(Debug, strum::EnumString, strum::Display)]
enum ReportFailure {
    #[strum(serialize = "afrf")]
    AuthReportFailureFormat,
}

/// DNS record `_dmarc.{domain}`
#[derive(Debug)]
pub struct Record {
    version: Version,
    adkim: AlignmentMode,
    aspf: AlignmentMode,
    failure_report_options: Vec<FailureReportOption>,
    receiver_policy: ReceiverPolicy,
    receiver_policy_subdomain: Option<ReceiverPolicy>,
    percentage: u8, // 0-100
    report_failure: ReportFailure,
    report_interval: u32,
    // TODO: parse dmarc uri
    report_aggregate_feedback: Vec<String>,
    // TODO: parse dmarc uri
    report_specific_message: Vec<String>,
}

impl std::str::FromStr for Record {
    type Err = ParseError;

    #[allow(clippy::too_many_lines)]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut adkim = AlignmentMode::Relaxed;
        let mut aspf = AlignmentMode::Relaxed;
        let mut failure_report_options = vec![FailureReportOption::All];
        let mut receiver_policy = None;
        let mut receiver_policy_subdomain = None;
        let mut percentage = 100u8;
        let mut version = None;
        let mut report_failure = ReportFailure::AuthReportFailureFormat;
        let mut report_interval = 86400;
        let mut report_aggregate_feedback = vec![];
        let mut report_specific_message = vec![];

        for i in s
            .split(';')
            .map(|tag| tag.split_whitespace().collect::<Vec<_>>().concat())
            // ?
            .take_while(|s| !s.is_empty())
        {
            match i.split_once('=').ok_or(ParseError::SyntaxError {
                reason: "tag syntax is `{tag}={value}`".to_string(),
            })? {
                ("v", p_version) => {
                    version = Some(Version::from_str(p_version).map_err(|e| {
                        ParseError::SyntaxError {
                            reason: format!("when parsing `version`, got: `{e}`"),
                        }
                    })?);
                }
                ("adkim", p_adkim) => {
                    adkim = p_adkim.parse().map_err(|e| ParseError::InvalidArgument {
                        reason: format!("invalid value for `adkim`: `{e}`"),
                    })?;
                }
                ("aspf", p_aspf) => {
                    aspf = p_aspf.parse().map_err(|e| ParseError::InvalidArgument {
                        reason: format!("invalid value for `aspf`: `{e}`"),
                    })?;
                }
                ("fo", p_fo) => {
                    failure_report_options = p_fo
                        .split(':')
                        .map(FailureReportOption::from_str)
                        .filter_map(Result::ok)
                        .collect::<Vec<_>>();
                }
                ("p", p_receiver_policy) => {
                    receiver_policy = Some(p_receiver_policy.parse().map_err(|e| {
                        ParseError::InvalidArgument {
                            reason: format!("invalid value for `p`: `{e}`"),
                        }
                    })?);
                }
                ("sp", p_receiver_policy_subdomain) => {
                    receiver_policy_subdomain =
                        Some(p_receiver_policy_subdomain.parse().map_err(|e| {
                            ParseError::InvalidArgument {
                                reason: format!("invalid value for `p`: `{e}`"),
                            }
                        })?);
                }
                ("pct", p_pct) => {
                    let i = p_pct
                        .parse::<i16>()
                        .map_err(|e| ParseError::InvalidArgument {
                            reason: format!("invalid value for `pct`: `{e}`"),
                        })?;

                    if i.clamp(0, 100) != i {
                        return Err(ParseError::InvalidArgument {
                            reason: format!(
                                "invalid value for `pct` ({i}): `must be between 0 and 100`"
                            ),
                        });
                    }

                    percentage = u8::try_from(i).expect("i16 is within u8 range");
                }
                ("rf", p_report_failure) => {
                    report_failure =
                        p_report_failure
                            .parse()
                            .map_err(|e| ParseError::InvalidArgument {
                                reason: format!("invalid value for `rf`: `{e}`"),
                            })?;
                }
                ("ri", p_report_interval) => {
                    report_interval = p_report_interval.parse::<u32>().map_err(|e| {
                        ParseError::InvalidArgument {
                            reason: format!("invalid value for `ri`: `{e}`"),
                        }
                    })?;
                }
                ("rua", p_report_aggregate_feedback) => {
                    report_aggregate_feedback = p_report_aggregate_feedback
                        .split(',')
                        .map(str::to_string)
                        .collect();
                }
                ("ruf", p_report_specific_message) => {
                    report_specific_message = p_report_specific_message
                        .split(',')
                        .map(str::to_string)
                        .collect();
                }
                // ignore unknown tag
                _ => continue,
            }
        }

        Ok(Self {
            version: version.ok_or(ParseError::MissingRequiredField {
                field: "v".to_string(),
            })?,
            adkim,
            aspf,
            failure_report_options,
            receiver_policy: receiver_policy.ok_or_else(|| ParseError::MissingRequiredField {
                field: "p".to_string(),
            })?,
            receiver_policy_subdomain,
            percentage,
            report_failure,
            report_interval,
            report_aggregate_feedback,
            report_specific_message,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn parse_outlook_fr() {
        let record =
            "v=DMARC1; p=none; rua=mailto:d@rua.agari.com;ruf=mailto:d@ruf.agari.com;fo=1:s:d";

        let record = Record::from_str(record).unwrap();
        println!("{record:#?}");
    }
}
