use crate::{Key, Signature};
use trust_dns_resolver::config::ResolverOpts;
use vsmtp_common::MessageBody;

const MAIL: &str = include_str!("simple.eml");

const SIGNATURE: &str = concat!(
    "DKIM-Signature: v=1; a=rsa-sha256; c=relaxed/relaxed;\r\n",
    "  d=epitechfr.onmicrosoft.com; s=selector2-epitechfr-onmicrosoft-com;\r\n",
    "  h=From:Date:Subject:Message-ID:Content-Type:MIME-Version:X-MS-Exchange-SenderADCheck;\r\n",
    "  bh=rtTGBOOAnprlA4aIQC8PvKyqp82URQPSnYcl/gjOxGk=;\r\n",
    "  b=Ucs4om63ogXgJNlwU2a/D4pANfDisgO72p9tEFI4smwNnK7IK8S61zCey9pKXob+CtxXhSvUZXE9lLE9Ta/0YdZ7ZsmExdzlzuV3hBtCnJPsSw0GVeHDLVSQx02YfZddfVOPTDn57T7CtnkiortgcPtOk0oeMn3Wv3JksDeQyOE=",
);

#[tokio::test]
async fn verify_with_raw_message() {
    let body = MessageBody::try_from(MAIL).unwrap();

    let resolver = trust_dns_resolver::TokioAsyncResolver::tokio(
        trust_dns_resolver::config::ResolverConfig::google(),
        ResolverOpts::default(),
    )
    .unwrap();

    let signature = <Signature as std::str::FromStr>::from_str(SIGNATURE).unwrap();
    let public_key = signature.get_public_key(&resolver).await.unwrap();
    let field = public_key.iter().next().unwrap();

    let public_key = <Key as std::str::FromStr>::from_str(&field.to_string()).unwrap();

    signature.verify(body.inner(), &public_key).unwrap();
}
