use crate::tests::tls::{get_tls_config, test_tls_tunneled};
use vsmtp_config::get_rustls_config;
use vsmtp_config::re::rustls;
use vsmtp_server::re::tokio;
use vsmtp_server::IoService;

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn test_all_cipher_suite() {
    for i in rustls::ALL_CIPHER_SUITES {
        let mut config = get_tls_config();
        println!("{i:?} {}", i.suite().get_u16(),);

        config.server.tls.as_mut().unwrap().protocol_version = vec![
            rustls::ProtocolVersion::TLSv1_2,
            rustls::ProtocolVersion::TLSv1_3,
        ];
        config.server.tls.as_mut().unwrap().cipher_suite = vec![i.suite()];

        let (client, server) = test_tls_tunneled(
            "testserver.com",
            std::sync::Arc::new(config),
            vec!["QUIT\r\n".to_string()],
            [
                "220 testserver.com Service ready",
                "221 Service closing transmission channel",
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
            19980 + u32::from(i.suite().get_u16()) % 100,
            |config| {
                Some(std::sync::Arc::new(
                    get_rustls_config(
                        config.server.tls.as_ref().unwrap(),
                        &config.server.r#virtual,
                    )
                    .unwrap(),
                ))
            },
            |_| None,
            |io: &IoService<rustls::Stream<rustls::ClientConnection, std::net::TcpStream>>| {
                assert_eq!(
                    i.suite(),
                    io.inner.conn.negotiated_cipher_suite().unwrap().suite()
                );
            },
        )
        .await
        .unwrap();

        client.unwrap();
        server.unwrap();
    }
}
