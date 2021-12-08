#![no_main]
use libfuzzer_sys::fuzz_target;

use std::{collections::HashMap};
use log::LevelFilter;

use v_smtp::{
    mailprocessing::mail_receiver::{MailReceiver, State},
    model::mail::MailContext,
    resolver::DataEndResolver,
    smtp::code::SMTPReplyCode,
    tests::Mock,
    config::server_config::{
        InnerLogConfig, InnerSMTPConfig, InnerSMTPErrorConfig, InnerServerConfig,
        InnerTlsConfig, ServerConfig, TlsSecurityLevel,
    },
};

struct DataEndResolverTest;
#[async_trait::async_trait]
impl DataEndResolver for DataEndResolverTest {
    async fn on_data_end(_: &ServerConfig, _: &MailContext) -> (State, SMTPReplyCode) {
        (State::MailFrom, SMTPReplyCode::Code250)
    }
}

fn get_test_config() -> ServerConfig {
    ServerConfig {
        domain: "{domain}".to_string(),
        version: "1.0.0".to_string(),
        server: InnerServerConfig { addr: vec![] },
        log: InnerLogConfig {
            file: "./tests/generated/output.log".to_string(),
            level: HashMap::<String, LevelFilter>::new(),
        },
        tls: InnerTlsConfig {
            security_level: TlsSecurityLevel::None,
            capath: None,
            preempt_cipherlist: true,
            handshake_timeout: std::time::Duration::from_millis(10_000),
            sni_maps: None,
        },
        smtp: InnerSMTPConfig {
            spool_dir: "./tests/generated/spool/".to_string(),
            timeout_client: HashMap::<String, String>::new(),
            error: InnerSMTPErrorConfig {
                soft_count: -1,
                hard_count: 10,
                delay: std::time::Duration::from_millis(0),
            },
        },
    }
}

fuzz_target!(|data: &[u8]| {
    let mut write_vec = Vec::new();
    let mut mock = Mock::new(data.to_vec(), &mut write_vec);
    let mut receiver = MailReceiver::<DataEndResolverTest>::new(
        "0.0.0.0:0".parse().unwrap(),
        None,
        std::sync::Arc::new(get_test_config()),
    );
    let future = receiver.receive_plain(&mut mock);

    let _future_result = match tokio::runtime::Handle::try_current() {
        Err(_) => match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime.block_on(future),
            Err(_) => todo!(),
        },
        Ok(handle) => handle.block_on(future),
    };
});
