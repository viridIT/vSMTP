use crate::{
    config::{
        log::{RECEIVER, RULES},
        server_config::TlsSecurityLevel,
    },
    connection::Connection,
    io_service::ReadError,
    model::{
        envelop::Envelop,
        mail::{MailContext, MAIL_CAPACITY},
    },
    rules::{
        address::Address,
        rule_engine::{RuleEngine, Status},
    },
    smtp::{code::SMTPReplyCode, event::Event, state::StateSMTP},
};

const TIMEOUT_DEFAULT: u64 = 10_000; // 10s

enum ProcessedEvent {
    Nothing,
    Reply(SMTPReplyCode),
    ReplyChangeState(StateSMTP, SMTPReplyCode),
    TransactionCompleted(MailContext),
}

pub struct Transaction<'re> {
    state: StateSMTP,
    mail: MailContext,
    rule_engine: RuleEngine<'re>,
}

pub enum TransactionResult {
    Nothing,
    Mail(MailContext),
    TlsUpgrade,
}

impl Transaction<'_> {
    fn parse_and_apply_and_get_reply<S: std::io::Read + std::io::Write>(
        &mut self,
        conn: &Connection<S>,
        client_message: String,
    ) -> ProcessedEvent {
        log::trace!(target: RECEIVER, "buffer=\"{}\"", client_message);

        let command_or_code = if self.state == StateSMTP::Data {
            Event::parse_data
        } else {
            Event::parse_cmd
        }(&client_message);

        log::trace!(target: RECEIVER, "parsed=\"{:?}\"", command_or_code);

        command_or_code
            .map(|command| self.process_event(conn, command))
            .unwrap_or_else(ProcessedEvent::Reply)
    }

    fn process_event<S: std::io::Read + std::io::Write>(
        &mut self,
        conn: &Connection<S>,
        event: Event,
    ) -> ProcessedEvent {
        match (&self.state, event) {
            (_, Event::NoopCmd) => ProcessedEvent::Reply(SMTPReplyCode::Code250),

            (_, Event::HelpCmd(_)) => ProcessedEvent::Reply(SMTPReplyCode::Code214),

            (_, Event::RsetCmd) => {
                self.mail.body = String::with_capacity(MAIL_CAPACITY);
                self.mail.envelop.rcpt.clear();
                self.mail.envelop.mail_from = Address::default();
                self.rule_engine.reset();

                ProcessedEvent::ReplyChangeState(StateSMTP::Helo, SMTPReplyCode::Code250)
            }

            (_, Event::ExpnCmd(_) | Event::VrfyCmd(_) /*| Event::PrivCmd*/) => {
                ProcessedEvent::Reply(SMTPReplyCode::Code502unimplemented)
            }

            (_, Event::QuitCmd) => {
                ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code221)
            }

            (_, Event::HeloCmd(helo)) => {
                self.set_helo(helo);
                log::trace!(target: RECEIVER, "envelop=\"{:?}\"", self.mail.envelop,);

                match self.rule_engine.run_when("helo") {
                    Status::Deny => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code554)
                    }
                    _ => ProcessedEvent::ReplyChangeState(StateSMTP::Helo, SMTPReplyCode::Code250),
                }
            }

            (_, Event::EhloCmd(_)) if conn.config.smtp.disable_ehlo => {
                ProcessedEvent::Reply(SMTPReplyCode::Code502unimplemented)
            }

            (_, Event::EhloCmd(helo)) => {
                self.set_helo(helo);
                log::trace!(target: RECEIVER, "envelop=\"{:?}\"", self.mail.envelop,);

                match self.rule_engine.run_when("helo") {
                    Status::Deny => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code554)
                    }
                    _ => ProcessedEvent::ReplyChangeState(
                        StateSMTP::Helo,
                        if conn.is_secured {
                            SMTPReplyCode::Code250SecuredEsmtp
                        } else {
                            SMTPReplyCode::Code250PlainEsmtp
                        },
                    ),
                }
            }

            (StateSMTP::Helo, Event::StartTls)
                if conn.config.tls.security_level != TlsSecurityLevel::None =>
            {
                ProcessedEvent::ReplyChangeState(StateSMTP::NegotiationTLS, SMTPReplyCode::Code220)
            }
            (StateSMTP::Helo, Event::StartTls)
                if conn.config.tls.security_level == TlsSecurityLevel::None =>
            {
                ProcessedEvent::Reply(SMTPReplyCode::Code454)
            }

            (StateSMTP::Helo, Event::MailCmd(_, _))
                if conn.config.tls.security_level == TlsSecurityLevel::Encrypt
                    && !conn.is_secured =>
            {
                ProcessedEvent::Reply(SMTPReplyCode::Code530)
            }

            (StateSMTP::Helo, Event::MailCmd(mail_from, _body_bit_mime)) => {
                // TODO: store in envelop _body_bit_mime

                self.mail.body = String::with_capacity(MAIL_CAPACITY);
                self.set_mail_from(mail_from);

                log::trace!(target: RECEIVER, "envelop=\"{:?}\"", self.mail.envelop,);

                match self.rule_engine.run_when("mail") {
                    Status::Deny => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code554)
                    }
                    _ => ProcessedEvent::ReplyChangeState(
                        StateSMTP::MailFrom,
                        SMTPReplyCode::Code250,
                    ),
                }
            }

            (StateSMTP::MailFrom | StateSMTP::RcptTo, Event::RcptCmd(rcpt_to)) => {
                self.set_rcpt_to(rcpt_to);

                log::trace!(target: RECEIVER, "envelop=\"{:?}\"", self.mail.envelop,);

                match self.rule_engine.run_when("rcpt") {
                    Status::Deny => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::Stop, SMTPReplyCode::Code554)
                    }
                    _ if self.mail.envelop.rcpt.len()
                        >= conn.config.smtp.rcpt_count_max.unwrap_or(usize::MAX) =>
                    {
                        ProcessedEvent::ReplyChangeState(
                            StateSMTP::RcptTo,
                            SMTPReplyCode::Code452TooManyRecipients,
                        )
                    }
                    _ => {
                        ProcessedEvent::ReplyChangeState(StateSMTP::RcptTo, SMTPReplyCode::Code250)
                    }
                }
            }

            (StateSMTP::RcptTo, Event::DataCmd) => {
                ProcessedEvent::ReplyChangeState(StateSMTP::Data, SMTPReplyCode::Code354)
            }

            (StateSMTP::Data, Event::DataLine(line)) => {
                self.mail.body.push_str(&line);
                self.mail.body.push('\n');
                ProcessedEvent::Nothing
            }

            (StateSMTP::Data, Event::DataEnd) => {
                self.rule_engine.add_data("data", self.mail.body.clone());

                let status = self.rule_engine.run_when("preq");

                if let Status::Block | Status::Deny = status {
                    return ProcessedEvent::ReplyChangeState(
                        StateSMTP::Stop,
                        SMTPReplyCode::Code554,
                    );
                }

                // executing all registered extensive operations.
                if let Err(error) = self.rule_engine.execute_operation_queue(
                    &self.mail,
                    &format!(
                        "{}_{:?}",
                        self.mail
                            .timestamp
                            .unwrap()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_millis(),
                        std::thread::current().id()
                    ),
                ) {
                    log::error!(
                        target: RULES,
                        "failed to empty the operation queue: '{}'",
                        error
                    );
                }

                // getting the server's envelop, that could have mutated in the
                // rule engine.
                if let Some(envelop) = self.rule_engine.get_scoped_envelop() {
                    self.mail.envelop = envelop;

                    let mut output = MailContext {
                        envelop: Envelop::default(),
                        body: String::with_capacity(MAIL_CAPACITY),
                        timestamp: None,
                    };

                    std::mem::swap(&mut self.mail, &mut output);

                    ProcessedEvent::TransactionCompleted(output)
                } else {
                    ProcessedEvent::ReplyChangeState(StateSMTP::MailFrom, SMTPReplyCode::Code554)
                }
            }

            _ => ProcessedEvent::Reply(SMTPReplyCode::Code503),
        }
    }
}

impl Transaction<'_> {
    fn set_connect<S: std::io::Read + std::io::Write>(&mut self, conn: &Connection<S>) {
        self.rule_engine.add_data("connect", conn.client_addr.ip());
        self.rule_engine.add_data("port", conn.client_addr.port());
        self.rule_engine
            .add_data("connection_timestamp", conn.timestamp);
    }

    fn set_helo(&mut self, helo: String) {
        self.mail.envelop = Envelop {
            helo,
            mail_from: Address::default(),
            rcpt: std::collections::HashSet::default(),
        };
        self.rule_engine.reset();

        self.rule_engine
            .add_data("helo", self.mail.envelop.helo.clone());
    }

    fn set_mail_from(&mut self, mail_from: String) {
        if let Ok(mail_from) = Address::new(&mail_from) {
            self.mail.envelop.mail_from = mail_from;
            self.mail.timestamp = Some(std::time::SystemTime::now());
            self.mail.envelop.rcpt.clear();
            self.rule_engine.reset();

            self.rule_engine
                .add_data("mail", self.mail.envelop.mail_from.clone());
            self.rule_engine
                .add_data("mail_timestamp", self.mail.timestamp);
        }
    }

    // FIXME: too many clone
    fn set_rcpt_to(&mut self, rcpt_to: String) {
        if let Ok(rcpt_to) = Address::new(&rcpt_to) {
            self.rule_engine.add_data("rcpt", rcpt_to.clone());

            match self
                .rule_engine
                .get_data::<std::collections::HashSet<Address>>("rcpts")
            {
                Some(mut rcpts) => {
                    rcpts.insert(rcpt_to);
                    self.mail.envelop.rcpt = rcpts.clone();
                    self.rule_engine.add_data("rcpts", rcpts.clone());
                }
                None => unreachable!("rcpts is injected by the default scope"),
            };
        } else {
            log::error!(target: RECEIVER, "rcpt's email address is invalid.");
        }
    }
}

impl Transaction<'_> {
    pub async fn receive<'a, 'b, S: std::io::Read + std::io::Write>(
        conn: &'a mut Connection<'b, S>,
        helo_domain: &Option<String>,
    ) -> std::io::Result<TransactionResult> {
        // TODO: move that cleanly in config
        let smtp_timeouts = conn
            .config
            .smtp
            .timeout_client
            .iter()
            .filter_map(|(k, v)| match humantime::parse_duration(v) {
                Ok(v) => Some((*k, v)),
                Err(e) => {
                    log::error!(
                        target: RECEIVER,
                        "error \"{}\" parsing timeout for key={}, ignored",
                        e,
                        k
                    );
                    None
                }
            })
            .collect::<std::collections::HashMap<_, _>>();

        let mut transaction = Transaction {
            state: if helo_domain.is_none() {
                StateSMTP::Connect
            } else {
                StateSMTP::Helo
            },
            mail: MailContext {
                envelop: Envelop::default(),
                body: String::with_capacity(MAIL_CAPACITY),
                timestamp: None,
            },
            rule_engine: RuleEngine::new(conn.config.as_ref()),
        };

        transaction.set_connect(conn);

        if let Some(helo) = helo_domain.as_ref().cloned() {
            transaction.set_helo(helo)
        }

        if let Status::Deny = transaction.rule_engine.run_when("connect") {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "connection at '{}' has been denied when connecting.",
                    conn.client_addr
                ),
            ));
        };

        let mut read_timeout = *smtp_timeouts
            .get(&transaction.state)
            .unwrap_or(&std::time::Duration::from_millis(TIMEOUT_DEFAULT));

        while transaction.state != StateSMTP::Stop {
            if transaction.state == StateSMTP::NegotiationTLS {
                return Ok(TransactionResult::TlsUpgrade);
            }
            match conn.read(read_timeout).await {
                Ok(Ok(client_message)) => {
                    match transaction.parse_and_apply_and_get_reply(conn, client_message) {
                        ProcessedEvent::Reply(reply_to_send) => {
                            conn.send_code(reply_to_send)?;
                        }
                        ProcessedEvent::ReplyChangeState(new_state, reply_to_send) => {
                            log::info!(
                                target: RECEIVER,
                                "================ STATE: /{:?}/ => /{:?}/",
                                transaction.state,
                                new_state
                            );
                            transaction.state = new_state;
                            read_timeout = *smtp_timeouts
                                .get(&transaction.state)
                                .unwrap_or(&std::time::Duration::from_millis(TIMEOUT_DEFAULT));
                            conn.send_code(reply_to_send)?;
                        }
                        ProcessedEvent::TransactionCompleted(mail) => {
                            return Ok(TransactionResult::Mail(mail))
                        }
                        ProcessedEvent::Nothing => {}
                    }
                }
                Ok(Err(ReadError::Blocking)) => {}
                Ok(Err(ReadError::Eof)) => {
                    log::info!(target: RECEIVER, "eof");
                    transaction.state = StateSMTP::Stop;
                }
                Ok(Err(ReadError::Other(e))) => {
                    // TODO: send error to client ?
                    return Err(e);
                }
                Err(e) => {
                    conn.send_code(SMTPReplyCode::Code451Timeout)?;
                    return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, e));
                }
            }
        }

        conn.is_alive = false;
        Ok(TransactionResult::Nothing)
    }
}
