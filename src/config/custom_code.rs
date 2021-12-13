use crate::{config::default::DEFAULT_CONFIG, smtp::code::SMTPReplyCode};

use super::server_config::ServerConfig;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CustomSMTPCode {
    code214: String,
    code220: String,
    code221: String,
    code250: String,
    code250_plain_esmtp: String,
    code250_secured_esmtp: String,
    code354: String,
    code451: String,
    code451_timeout: String,
    code451_too_many_error: String,
    code452: String,
    code452_too_many_recipients: String,
    code454: String,
    code500: String,
    code501: String,
    code502_unimplemented: String,
    code503: String,
    code530: String,
    code554: String,
    code554tls: String,
}

impl CustomSMTPCode {
    pub fn get(&self, code: &SMTPReplyCode) -> &String {
        match code {
            SMTPReplyCode::Code214 => &self.code214,
            SMTPReplyCode::Code220 => &self.code220,
            SMTPReplyCode::Code221 => &self.code221,
            SMTPReplyCode::Code250 => &self.code250,
            SMTPReplyCode::Code250PlainEsmtp => &self.code250_plain_esmtp,
            SMTPReplyCode::Code250SecuredEsmtp => &self.code250_secured_esmtp,
            SMTPReplyCode::Code354 => &self.code354,
            SMTPReplyCode::Code451 => &self.code451,
            SMTPReplyCode::Code451Timeout => &self.code451_timeout,
            SMTPReplyCode::Code451TooManyError => &self.code451_too_many_error,
            SMTPReplyCode::Code452 => &self.code452,
            SMTPReplyCode::Code452TooManyRecipients => &self.code452_too_many_recipients,
            SMTPReplyCode::Code454 => &self.code454,
            SMTPReplyCode::Code500 => &self.code500,
            SMTPReplyCode::Code501 => &self.code501,
            SMTPReplyCode::Code502unimplemented => &self.code502_unimplemented,
            SMTPReplyCode::Code503 => &self.code503,
            SMTPReplyCode::Code530 => &self.code530,
            SMTPReplyCode::Code554 => &self.code554,
            SMTPReplyCode::Code554tls => &self.code554tls,
        }
    }

    pub(super) fn from_raw(
        raw: &std::collections::HashMap<SMTPReplyCode, String>,
        config: &ServerConfig,
        prepare_for_default: bool,
    ) -> Self {
        let get = if prepare_for_default {
            |raw: &std::collections::HashMap<SMTPReplyCode, String>,
             _config: &ServerConfig,
             code: &SMTPReplyCode|
             -> String {
                match raw.get(code) {
                    Some(code) => code.clone(),
                    None => panic!("missing code in default config {:?}", code),
                }
            }
        } else {
            |raw: &std::collections::HashMap<SMTPReplyCode, String>,
             config: &ServerConfig,
             code: &SMTPReplyCode|
             -> String {
                raw.get(code)
                    .unwrap_or_else(|| DEFAULT_CONFIG.smtp.get_code().get(code))
                    .clone()
                    .replace("{domain}", &config.domain)
            }
        };

        Self {
            code214: get(raw, config, &SMTPReplyCode::Code214),
            code220: get(raw, config, &SMTPReplyCode::Code220),
            code221: get(raw, config, &SMTPReplyCode::Code221),
            code250: get(raw, config, &SMTPReplyCode::Code250),
            code250_plain_esmtp: get(raw, config, &SMTPReplyCode::Code250PlainEsmtp),
            code250_secured_esmtp: get(raw, config, &SMTPReplyCode::Code250SecuredEsmtp),
            code354: get(raw, config, &SMTPReplyCode::Code354),
            code451: get(raw, config, &SMTPReplyCode::Code451),
            code451_timeout: get(raw, config, &SMTPReplyCode::Code451Timeout),
            code451_too_many_error: get(raw, config, &SMTPReplyCode::Code451TooManyError),
            code452: get(raw, config, &SMTPReplyCode::Code452),
            code452_too_many_recipients: get(raw, config, &SMTPReplyCode::Code452TooManyRecipients),
            code454: get(raw, config, &SMTPReplyCode::Code454),
            code500: get(raw, config, &SMTPReplyCode::Code500),
            code501: get(raw, config, &SMTPReplyCode::Code501),
            code502_unimplemented: get(raw, config, &SMTPReplyCode::Code502unimplemented),
            code503: get(raw, config, &SMTPReplyCode::Code503),
            code530: get(raw, config, &SMTPReplyCode::Code530),
            code554: get(raw, config, &SMTPReplyCode::Code554),
            code554tls: get(raw, config, &SMTPReplyCode::Code554tls),
        }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum SMTPCode {
    Raw(std::collections::HashMap<SMTPReplyCode, String>),
    Serialized(Box<CustomSMTPCode>),
}
