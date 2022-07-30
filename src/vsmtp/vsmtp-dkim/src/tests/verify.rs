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

fn verify_3rd_party(filepath: &str) {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("dkimverify -v < {filepath}"))
        .output()
        .expect("failed to execute process");

    assert_eq!(
        output,
        std::process::Output {
            status: <std::process::ExitStatus as std::os::unix::prelude::ExitStatusExt>::from_raw(
                0
            ),
            stdout: b"signature ok\n".to_vec(),
            stderr: vec![],
        }
    );
}

#[tokio::test]
async fn mail_1() {
    verify(include_str!("mail_1.eml")).await;
}

#[tokio::test]
async fn mail_2() {
    verify(&include_str!("mail_2.eml").replace('\n', "\r\n")).await;
}

#[tokio::test]
async fn mail_3() {
    verify(&include_str!("mail_3.eml").replace('\n', "\r\n")).await;
}

#[test]
#[ignore = "need `sudo apt install python3-dkim`"]
fn mail_1_3rd_party() {
    verify_3rd_party("./src/tests/mail_1.eml");
}

#[test]
#[ignore = "need `sudo apt install python3-dkim`"]
fn mail_2_3rd_party() {
    verify_3rd_party("./src/tests/mail_2.eml");
}

#[test]
#[ignore = "need `sudo apt install python3-dkim`"]
fn mail_3_3rd_party() {
    verify_3rd_party("./src/tests/mail_3.eml");
}
