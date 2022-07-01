use trust_dns_resolver::config::ResolverOpts;
use vsmtp_common::{Either, MailParser, MessageBody, ParserOutcome, RawBody};
use vsmtp_mail_parser::MailMimeParser;

use crate::{verify, Key, Signature};

const MAIL: &str = include_str!("simple.eml");

const SIGNATURE: &str = concat!(
    "DKIM-Signature: v=1; a=rsa-sha256; c=relaxed/relaxed;\r\n",
    "  d=epitechfr.onmicrosoft.com; s=selector2-epitechfr-onmicrosoft-com;\r\n",
    "  h=From:Date:Subject:Message-ID:Content-Type:MIME-Version:X-MS-Exchange-SenderADCheck;\r\n",
    "  bh=rtTGBOOAnprlA4aIQC8PvKyqp82URQPSnYcl/gjOxGk=;\r\n",
    "  b=Ucs4om63ogXgJNlwU2a/D4pANfDisgO72p9tEFI4smwNnK7IK8S61zCey9pKXob+CtxXhSvUZXE9lLE9Ta/0YdZ7ZsmExdzlzuV3hBtCnJPsSw0GVeHDLVSQx02YfZddfVOPTDn57T7CtnkiortgcPtOk0oeMn3Wv3JksDeQyOE=",
);

#[derive(Default)]
struct NoParsing;

impl MailParser for NoParsing {
    fn parse_lines(&mut self, raw: &[&str]) -> ParserOutcome {
        let mut headers = Vec::<String>::new();
        let mut body = String::new();

        let mut stream = raw.into_iter();

        for line in stream.by_ref() {
            if line.is_empty() {
                break;
            }
            headers.push(line.to_string());
        }

        for line in stream {
            body.push_str(&line);
            body.push_str("\r\n");
        }

        Ok(Either::Left(RawBody {
            headers,
            body: Some(body),
        }))
    }
}

#[tokio::test]
async fn verify_with_raw_message() {
    let body = NoParsing::default()
        .parse_lines(&MAIL.lines().collect::<Vec<_>>())
        .unwrap()
        .unwrap_left();

    let resolver = trust_dns_resolver::TokioAsyncResolver::tokio(
        trust_dns_resolver::config::ResolverConfig::google(),
        ResolverOpts::default(),
    )
    .unwrap();

    let signature =
        <Signature as std::str::FromStr>::from_str(&SIGNATURE["DKIM-Signature: ".len()..]).unwrap();
    let public_key = signature.get_public_key(&resolver).await.unwrap();
    let field = public_key.iter().next().unwrap();

    let public_key = <Key as std::str::FromStr>::from_str(&field.to_string()).unwrap();

    verify(&body, &signature, &public_key).unwrap();
}

#[test]
fn prerequisite() {
    let parsed = MailParser::parse_lines(
        &mut MailMimeParser::default(),
        &MAIL.lines().collect::<Vec<_>>()[..],
    )
    .unwrap()
    .unwrap_right();

    pretty_assertions::assert_eq!(parsed.to_string(), MAIL);
}

// FIXME:
#[tokio::test]
async fn verify_with_parsed() {
    let body = NoParsing::default()
        .parse_lines(&MAIL.lines().collect::<Vec<_>>())
        .unwrap()
        .unwrap_left();

    let resolver = trust_dns_resolver::TokioAsyncResolver::tokio(
        trust_dns_resolver::config::ResolverConfig::google(),
        ResolverOpts::default(),
    )
    .unwrap();

    let signature =
        <Signature as std::str::FromStr>::from_str(&SIGNATURE["DKIM-Signature: ".len()..]).unwrap();
    let public_key = signature.get_public_key(&resolver).await.unwrap();
    let field = public_key.iter().next().unwrap();

    let public_key = <Key as std::str::FromStr>::from_str(&field.to_string()).unwrap();

    verify(&body, &signature, &public_key).unwrap();
}
