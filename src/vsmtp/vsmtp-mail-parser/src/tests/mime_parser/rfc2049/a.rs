use crate::MailMimeParser;
use vsmtp_common::{
    collection, BodyType, Mail, MailHeaders, MailParser, Mime, MimeBodyType, MimeHeader,
    MimeMultipart,
};

const MAIL: &str = include_str!("../../mail/rfc2049/A.eml");

#[test]
#[allow(clippy::too_many_lines)]
#[ignore]
fn simple() {
    let parsed = MailMimeParser::default()
        .parse_lines(&MAIL.lines().collect::<Vec<_>>())
        .unwrap()
        .unwrap_right();
    pretty_assertions::assert_eq!(
        parsed,
        Mail {
            headers: MailHeaders(
                [
                    ("mime-version", "1.0"),
                    ("from", "Nathaniel Borenstein <nsb@nsb.fv.com>"),
                    ("date", "Fri, 07 Oct 1994 16:15:05 -0700 (PDT)"),
                    ("to", "Ned Freed <ned@innosoft.com>"),
                    ("subject", "A multipart example"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
            ),
            body: BodyType::Mime(Box::new(Mime {
                headers: [MimeHeader {
                    name: "content-type".to_string(),
                    value: "multipart/mixed".to_string(),
                    args: collection! {
                        "boundary".to_string() => "unique-boundary-1".to_string(),
                    },
                },]
                .to_vec(),
                content: MimeBodyType::Multipart(MimeMultipart {
                    preamble: [
                        "This is the preamble area of a multipart message.\r\n",
                        "Mail readers that understand multipart format\r\n",
                        "should ignore this preamble.\r\n",
                        "\r\n",
                        "If you are reading this text, you might want to\r\n",
                        "consider changing to a mail reader that understands\r\n",
                        "how to properly display multipart messages.\r\n"
                    ]
                    .concat(),
                    parts: vec![
                        Mime {
                            headers: vec![],
                            content: MimeBodyType::Regular(
                                [
                                    "  ... Some text appears here ...",
                                    "",
                                    "[Note that the blank between the boundary and the start",
                                    " of the text in this part means no header fields were",
                                    " given and this is text in the US-ASCII character set.",
                                    " It could have been done with explicit typing as in the",
                                    " next part.]",
                                    "",
                                ]
                                .into_iter()
                                .map(str::to_string)
                                .collect::<Vec<_>>()
                            )
                        },
                        Mime {
                            headers: vec![MimeHeader {
                                name: "content-type".to_string(),
                                value: "text/plain".to_string(),
                                args: collection! {
                                    "charset".to_string() => "US-ASCII".to_string(),
                                },
                            },],
                            content: MimeBodyType::Regular(
                                [
                                    "This could have been part of the previous part, but",
                                    "illustrates explicit versus implicit typing of body",
                                    "parts.",
                                    "",
                                ]
                                .into_iter()
                                .map(str::to_string)
                                .collect::<Vec<_>>(),
                            ),
                        },
                        Mime {
                            headers: vec![MimeHeader {
                                name: "content-type".to_string(),
                                value: "multipart/parallel".to_string(),
                                args: collection! {
                                    "boundary".to_string()=> "unique-boundary-2".to_string(),
                                },
                            },],
                            content: MimeBodyType::Multipart(MimeMultipart {
                                preamble: "".to_string(),
                                parts: vec![
                                    Mime {
                                        headers: vec![
                                            MimeHeader {
                                                name: "content-type".to_string(),
                                                value: "audio/basic".to_string(),
                                                args: collection! {},
                                            },
                                            MimeHeader {
                                                name: "content-transfer-encoding".to_string(),
                                                value: "base64".to_string(),
                                                args: collection! {},
                                            },
                                        ],
                                        content: MimeBodyType::Regular(
                                            [
                                                "  ... base64-encoded 8000 Hz single-channel",
                                                "      mu-law-format audio data goes here ...",
                                                "",
                                            ]
                                            .into_iter()
                                            .map(str::to_string)
                                            .collect::<Vec<_>>(),
                                        )
                                    },
                                    Mime {
                                        headers: vec![
                                            MimeHeader {
                                                name: "content-type".to_string(),
                                                value: "image/jpeg".to_string(),
                                                args: collection! {},
                                            },
                                            MimeHeader {
                                                name: "content-transfer-encoding".to_string(),
                                                value: "base64".to_string(),
                                                args: collection! {},
                                            },
                                        ],
                                        content: MimeBodyType::Regular(
                                            ["  ... base64-encoded image data goes here ...", "",]
                                                .into_iter()
                                                .map(str::to_string)
                                                .collect::<Vec<_>>(),
                                        )
                                    }
                                ],
                                epilogue: "".to_string()
                            })
                        },
                        Mime {
                            headers: vec![MimeHeader {
                                name: "content-type".to_string(),
                                value: "text/enriched".to_string(),
                                args: collection! {},
                            },],
                            content: MimeBodyType::Regular(
                                [
                                    "This is <bold><italic>enriched.</italic></bold>",
                                    "<smaller>as defined in RFC 1896</smaller>",
                                    "",
                                    "Isn't it",
                                    "<bigger><bigger>cool?</bigger></bigger>",
                                    "",
                                ]
                                .into_iter()
                                .map(str::to_string)
                                .collect::<Vec<_>>(),
                            )
                        },
                        Mime {
                            headers: vec![MimeHeader {
                                name: "content-type".to_string(),
                                value: "message/rfc822".to_string(),
                                args: collection! {},
                            },],
                            content: MimeBodyType::Embedded(Mail {
                                headers: MailHeaders(
                                    [
                                        ("date", "(date in US-ASCII)"),
                                        ("from", "(mailbox in US-ASCII)"),
                                        ("to", "(address in US-ASCII)"),
                                        ("subject", "(subject in US-ASCII)"),
                                    ]
                                    .into_iter()
                                    .map(|(k, v)| (k.to_string(), v.to_string()))
                                    .collect::<Vec<_>>()
                                ),
                                // FIXME: line 68 and 69 are skipped (from .eml)
                                body: BodyType::Regular(
                                    ["  ... Additional text in ISO-8859-1 goes here ...", "",]
                                        .into_iter()
                                        .map(str::to_string)
                                        .collect::<Vec<_>>(),
                                )
                            })
                        }
                    ],
                    epilogue: "".to_string(),
                })
            }))
        }
    );

    pretty_assertions::assert_eq!(parsed.to_string(), MAIL.replace('\n', "\r\n"));
}
