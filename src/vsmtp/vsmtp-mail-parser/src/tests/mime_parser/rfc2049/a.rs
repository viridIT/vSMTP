use crate::MailMimeParser;
use vsmtp_common::{
    collection,
    mail_context::MessageBody,
    MailParser, {BodyType, Mail}, {Mime, MimeBodyType, MimeHeader, MimeMultipart},
};

const MAIL: &str = include_str!("../../mail/rfc2049/A.eml");

#[test]
#[allow(clippy::too_many_lines)]
fn simple() {
    let parsed = MailMimeParser::default()
        .parse(MAIL.lines().map(str::to_string).collect::<Vec<_>>())
        .unwrap();
    pretty_assertions::assert_eq!(
        parsed,
        MessageBody::Parsed(Box::new(Mail {
            headers: vec![
                ("mime-version", "1.0"),
                ("from", "Nathaniel Borenstein <nsb@nsb.fv.com>",),
                ("date", "Fri, 07 Oct 1994 16:15:05 -0700 ",),
                ("to", "Ned Freed <ned@innosoft.com>",),
                ("subject", "A multipart example",),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>(),
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
                                headers: [
                                    ("date", "",),
                                    ("from", "",),
                                    ("to", "",),
                                    ("subject", "",),
                                ]
                                .into_iter()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect::<Vec<_>>(),
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
        }))
    );

    pretty_assertions::assert_eq!(
        parsed.to_string(),
        [
            "mime-version: 1.0\r\n".to_string(),
            "from: Nathaniel Borenstein <nsb@nsb.fv.com>\r\n".to_string(),
            "date: Fri, 07 Oct 1994 16:15:05 -0700 \r\n".to_string(),
            "to: Ned Freed <ned@innosoft.com>\r\n".to_string(),
            "subject: A multipart example\r\n".to_string(),
            "content-type: multipart/mixed; boundary=\"unique-boundary-1\"\r\n".to_string(),
            "\r\n".to_string(),
            "This is the preamble area of a multipart message.\r\n".to_string(),
            "Mail readers that understand multipart format\r\n".to_string(),
            "should ignore this preamble.\r\n".to_string(),
            "\r\n".to_string(),
            "If you are reading this text, you might want to\r\n".to_string(),
            "consider changing to a mail reader that understands\r\n".to_string(),
            "how to properly display multipart messages.\r\n".to_string(),
            "\r\n".to_string(),
            "--unique-boundary-1\r\n".to_string(),
            "\r\n".to_string(),
            "  ... Some text appears here ...\r\n".to_string(),
            "\r\n".to_string(),
            "[Note that the blank between the boundary and the start\r\n".to_string(),
            " of the text in this part means no header fields were\r\n".to_string(),
            " given and this is text in the US-ASCII character set.\r\n".to_string(),
            " It could have been done with explicit typing as in the\r\n".to_string(),
            " next part.]\r\n".to_string(),
            "\r\n".to_string(),
            "--unique-boundary-1\r\n".to_string(),
            "content-type: text/plain; charset=\"US-ASCII\"\r\n".to_string(),
            "\r\n".to_string(),
            "This could have been part of the previous part, but\r\n".to_string(),
            "illustrates explicit versus implicit typing of body\r\n".to_string(),
            "parts.\r\n".to_string(),
            "\r\n".to_string(),
            "--unique-boundary-1\r\n".to_string(),
            "content-type: multipart/parallel; boundary=\"unique-boundary-2\"\r\n".to_string(),
            "\r\n".to_string(),
            "--unique-boundary-2\r\n".to_string(),
            "content-type: audio/basic\r\n".to_string(),
            "content-transfer-encoding: base64\r\n".to_string(),
            "\r\n".to_string(),
            "  ... base64-encoded 8000 Hz single-channel\r\n".to_string(),
            "      mu-law-format audio data goes here ...\r\n".to_string(),
            "\r\n".to_string(),
            "--unique-boundary-2\r\n".to_string(),
            "content-type: image/jpeg\r\n".to_string(),
            "content-transfer-encoding: base64\r\n".to_string(),
            "\r\n".to_string(),
            "  ... base64-encoded image data goes here ...\r\n".to_string(),
            "\r\n".to_string(),
            "--unique-boundary-2--\r\n".to_string(),
            "\r\n".to_string(),
            "--unique-boundary-1\r\n".to_string(),
            "content-type: text/enriched\r\n".to_string(),
            "\r\n".to_string(),
            "This is <bold><italic>enriched.</italic></bold>\r\n".to_string(),
            "<smaller>as defined in RFC 1896</smaller>\r\n".to_string(),
            "\r\n".to_string(),
            "Isn't it\r\n".to_string(),
            "<bigger><bigger>cool?</bigger></bigger>\r\n".to_string(),
            "\r\n".to_string(),
            "--unique-boundary-1\r\n".to_string(),
            "content-type: message/rfc822\r\n".to_string(),
            "\r\n".to_string(),
            "date: \r\n".to_string(),
            "from: \r\n".to_string(),
            "to: \r\n".to_string(),
            "subject: \r\n".to_string(),
            // FIXME: line 68 and 69 are skipped (from .eml)
            // "content-type: Text/plain; charset=ISO-8859-1\r\n".to_string(),
            // "content-transfer-encoding: Quoted-printable\r\n".to_string(),
            "\r\n".to_string(),
            "  ... Additional text in ISO-8859-1 goes here ...\r\n".to_string(),
            "\r\n".to_string(),
            "--unique-boundary-1--\r\n".to_string(),
            // FIXME: remove this last line
            "\r\n".to_string()
        ]
        .concat()
    );
}
