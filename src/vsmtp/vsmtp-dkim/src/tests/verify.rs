use crate::{PublicKey, Signature};
use trust_dns_resolver::config::ResolverOpts;
use vsmtp_common::MessageBody;

#[tokio::test]
async fn simple() {
    const MAIL: &str = include_str!("simple.eml");

    let body = MessageBody::try_from(MAIL).unwrap();

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
#[ignore = "issue with public key ?"]
async fn simple2() {
    const MAIL: &str = include_str!("simple2.eml");

    let body = MessageBody::try_from(MAIL.replace('\n', "\r\n").as_str()).unwrap();

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
