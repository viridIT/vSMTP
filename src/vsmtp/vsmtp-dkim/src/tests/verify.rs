use crate::{PublicKey, Signature};
use trust_dns_resolver::config::ResolverOpts;
use vsmtp_common::MessageBody;

async fn verify(mail: &str) {
    let body = MessageBody::try_from(mail).unwrap();

    let resolver = trust_dns_resolver::TokioAsyncResolver::tokio(
        trust_dns_resolver::config::ResolverConfig::google(),
        ResolverOpts::default(),
    )
    .unwrap();

    let signature = <Signature as std::str::FromStr>::from_str(
        &body.inner().get_header("DKIM-Signature", true).unwrap(),
    )
    .unwrap();
    let public_key = resolver
        .txt_lookup(signature.get_dns_query())
        .await
        .unwrap();
    let field = public_key.iter().next().unwrap();

    let public_key = <PublicKey as std::str::FromStr>::from_str(&field.to_string()).unwrap();

    signature.verify(body.inner(), &public_key).unwrap();
}

#[tokio::test]
async fn mail_1() {
    verify(include_str!("mail_1.eml")).await;
}

#[tokio::test]
#[ignore]
async fn mail_2() {
    verify(&include_str!("mail_2.eml").replace('\n', "\r\n")).await;
}

#[tokio::test]
#[ignore]
async fn mail_3() {
    verify(&include_str!("mail_3.eml").replace('\n', "\r\n")).await;
}
