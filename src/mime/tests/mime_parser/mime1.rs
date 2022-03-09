use crate::mime::{
    mail::{BodyType, Mail},
    mime_type::{Mime, MimeBodyType, MimeHeader, MimeMultipart},
    parser::MailMimeParser,
};

const MAIL: &str = include_str!("../mail/mime1.eml");

#[allow(clippy::too_many_lines)]
#[test]
fn mime_parser() {
    assert_eq!(
        MailMimeParser::default().parse(MAIL.as_bytes()).unwrap(),
        Mail { headers:
            vec![
                ("from", "\"Sender Name\" <sender@example.com>"),
                ("to", "recipient@example.com"),
                ("subject", "Customer service contact info"),
                ("date", "Fri, 21 Nov 1997 09:55:06 -0600"),
                ("mime-version", "1.0")
            ].into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>(),
        body: BodyType::Mime(Box::new(Mime {
            headers: vec![
                MimeHeader {
                    name: "content-type".to_string(),
                    value: "multipart/mixed".to_string(),
                    args: crate::collection!{
                        "boundary".to_string() =>
                        "a3f166a86b56ff6c37755292d690675717ea3cd9de81228ec2b76ed4a15d6d1a".to_string()
                    }
                }
            ],
            content: MimeBodyType::Multipart(MimeMultipart {
                preamble: "".to_string(),
                parts: vec![
                    Mime {
                        headers: vec![
                            MimeHeader {
                                name: "content-type".to_string(),
                                value: "multipart/alternative".to_string(),
                                args: crate::collection!{
                                    "boundary".to_string() =>
                                    "sub_a3f166a86b56ff6c37755292d690675717ea3cd9de81228ec2b76ed4a15d6d1a".to_string()
                                }
                            }
                        ],
                        content: MimeBodyType::Multipart(MimeMultipart {
                            preamble: "".to_string(),
                            parts: vec![
                                Mime {
                                    headers: vec![
                                        MimeHeader {
                                            name: "content-type".to_string(),
                                            value: "text/plain".to_string(),
                                            args: crate::collection!{
                                                "charset".to_string() => "iso-8859-1".to_string()
                                            }
                                        },
                                        MimeHeader {
                                            name: "content-transfer-encoding".to_string(),
                                            value: "quoted-printable".to_string(),
                                            args: crate::collection!{}
                                        }
                                    ],
                                    content: MimeBodyType::Regular(vec![
                                        "Please see the attached file for a list of customers to contact.",
                                        ""
                                    ].into_iter().map(str::to_string).collect::<_>())
                                },
                                Mime {
                                    headers: vec![
                                        MimeHeader {
                                            name: "content-type".to_string(),
                                            value: "text/html".to_string(),
                                            args: crate::collection!{
                                                "charset".to_string() => "iso-8859-1".to_string()
                                            }
                                        },
                                        MimeHeader {
                                            name: "content-transfer-encoding".to_string(),
                                            value: "quoted-printable".to_string(),
                                            args: crate::collection!{}
                                        }
                                    ],
                                    content: MimeBodyType::Regular(vec![
                                        "<html>",
                                        "<head></head>",
                                        "<body>",
                                        "<h1>Hello!</h1>",
                                        "<p>Please see the attached file for a list of customers to contact.</p>",
                                        "</body>",
                                        "</html>",
                                        ""
                                    ].into_iter().map(str::to_string).collect::<_>())
                                }
                            ],
                            epilogue: "".to_string()
                        })
                    },
                    Mime {
                        headers: vec![
                            MimeHeader {
                                name: "content-type".to_string(),
                                value: "text/plain".to_string(),
                                args: crate::collection!{
                                    "name".to_string() => "customers.txt".to_string()
                                }
                            },
                            MimeHeader {
                                name: "content-description".to_string(),
                                value: "customers.txt".to_string(),
                                args: crate::collection!{}
                            },
                            MimeHeader {
                                name: "content-disposition".to_string(),
                                value: "attachment".to_string(),
                                args: crate::collection!{
                                    "filename".to_string() => "customers.txt".to_string(),
                                    "creation-date".to_string() => "Sat, 05 Aug 2017 19:35:36 GMT".to_string()
                                }
                            },
                            MimeHeader {
                                name: "content-transfer-encoding".to_string(),
                                value: "base64".to_string(),
                                args: crate::collection!{}
                            }
                        ],
                        content: MimeBodyType::Regular(vec![
                            "SUQsRmlyc3ROYW1lLExhc3ROYW1lLENvdW50cnkKMzQ4LEpvaG4sU3RpbGVzLENhbmFkYQo5MjM4",
                            "OSxKaWUsTGl1LENoaW5hCjczNCxTaGlybGV5LFJvZHJpZ3VleixVbml0ZWQgU3RhdGVzCjI4OTMs",
                            "QW5heWEsSXllbmdhcixJbmRpYQ==",
                            ""
                        ].into_iter().map(str::to_string).collect::<_>())
                    }],
                    epilogue: "".to_string()
                })
            }))
        }
    );
}
