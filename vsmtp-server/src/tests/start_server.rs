use vsmtp_rule_engine::rule_engine::RuleEngine;

use crate::{
    processes::ProcessMessage, receiver::test_helpers::get_regular_config, server::ServerVSMTP,
};

macro_rules! bind_address {
    ($addr:expr, $addr_submission:expr, $addr_submissions:expr) => {{
        let config = std::sync::Arc::new({
            let mut config = get_regular_config();
            config.server.interfaces.addr = vec!["0.0.0.0:10026".parse().unwrap()];
            config.server.interfaces.addr_submission = vec!["0.0.0.0:10588".parse().unwrap()];
            config.server.interfaces.addr_submissions = vec!["0.0.0.0:10466".parse().unwrap()];
            config
        });

        let (delivery_sender, _delivery_receiver) = tokio::sync::mpsc::channel::<ProcessMessage>(
            config.server.queues.delivery.channel_size,
        );

        let (working_sender, _working_receiver) =
            tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.working.channel_size);

        let rule_engine =
            std::sync::Arc::new(std::sync::RwLock::new(RuleEngine::new(&None).unwrap()));

        let s = ServerVSMTP::new(
            config.clone(),
            (
                std::net::TcpListener::bind(&config.server.interfaces.addr[..]).unwrap(),
                std::net::TcpListener::bind(&config.server.interfaces.addr_submission[..]).unwrap(),
                std::net::TcpListener::bind(&config.server.interfaces.addr_submissions[..])
                    .unwrap(),
            ),
            rule_engine,
            working_sender,
            delivery_sender,
        )
        .unwrap();

        assert_eq!(
            s.addr(),
            [
                config.server.interfaces.addr.clone(),
                config.server.interfaces.addr_submission.clone(),
                config.server.interfaces.addr_submissions.clone()
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
        );
    }};
}

#[tokio::test]
async fn init() {
    bind_address! {
        vec!["0.0.0.0:10026".parse().unwrap()],
        vec!["0.0.0.0:10588".parse().unwrap()],
        vec!["0.0.0.0:10466".parse().unwrap()]
    }
}
