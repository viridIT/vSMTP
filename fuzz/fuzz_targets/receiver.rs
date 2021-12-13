#![no_main]
use libfuzzer_sys::fuzz_target;

use vsmtp::{
    config::server_config::ServerConfig,
    mailprocessing::mail_receiver::{MailReceiver, StateSMTP},
    model::mail::MailContext,
    resolver::DataEndResolver,
    smtp::code::SMTPReplyCode,
    tests::Mock,
};

struct DataEndResolverTest;
#[async_trait::async_trait]
impl DataEndResolver for DataEndResolverTest {
    async fn on_data_end(_: &ServerConfig, _: &MailContext) -> (StateSMTP, SMTPReplyCode) {
        (StateSMTP::MailFrom, SMTPReplyCode::Code250)
    }
}

fuzz_target!(|data: &[u8]| {
    let mut config: ServerConfig =
        toml::from_str(include_str!("fuzz.config.toml")).expect("cannot parse config from toml");
    config.prepare();

    let mut write_vec = Vec::new();
    let mut mock = Mock::new(data.to_vec(), &mut write_vec);
    let mut receiver = MailReceiver::<DataEndResolverTest>::new(
        "0.0.0.0:0".parse().unwrap(),
        None,
        std::sync::Arc::new(config),
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
