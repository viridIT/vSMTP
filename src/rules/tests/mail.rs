/**
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
**/
#[cfg(test)]
pub mod test {
    use crate::{
        mime::parser::MailMimeParser,
        rules::{
            address::Address,
            rule_engine::{RuleEngine, Status},
            tests::helpers::get_default_state,
        },
        smtp::mail::Body,
    };

    #[tokio::test]
    async fn test_mail_from_rules() {
        {
            let mut config = crate::receiver::test_helpers::get_regular_config().unwrap();
            config
                .log
                .level
                .insert("rules".into(), log::LevelFilter::Trace);
            config
                .log
                .level
                .insert("mail_parser".into(), log::LevelFilter::Trace);
            log4rs::init_config(crate::config::get_logger_config(&config).unwrap()).unwrap();
        }

        let re =
            RuleEngine::new("./src/rules/tests/rules/mail").expect("couldn't build rule engine");

        let mut state = get_default_state();
        {
            let email = state.get_context();
            let mut email = email.write().unwrap();

            email.envelop.mail_from = Address::new("staff@viridit.com").unwrap();
            email.body = Body::Parsed(Box::new(
                MailMimeParser::default()
                    .parse(
                        br#"From: staff <staff@viridit.com>
Date: Fri, 21 Nov 1997 10:01:10 -0600

This is a reply to your hello."#,
                    )
                    .unwrap(),
            ));
        }

        assert_eq!(re.run_when(&mut state, "mail"), Status::Accept);
        assert_eq!(re.run_when(&mut state, "postq"), Status::Accept);
        assert_eq!(
            state.get_context().read().unwrap().envelop.mail_from.full(),
            "no-reply@viridit.com"
        );
    }
}
