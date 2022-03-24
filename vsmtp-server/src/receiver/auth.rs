use super::Connection;
use crate::server::SaslBackend;
use vsmtp_common::{auth::Mechanism, code::SMTPReplyCode, re::rsasl};

fn auth_step<S>(
    conn: &mut Connection<'_, S>,
    session: &mut rsasl::DiscardOnDrop<rsasl::Session<()>>,
    buffer: String,
) -> anyhow::Result<bool>
where
    S: std::io::Read + std::io::Write + Send,
{
    let bytes64decoded = base64::decode(buffer).unwrap(); // 501 5.5.2

    match session.step(&bytes64decoded) {
        Ok(rsasl::Step::Done(buffer)) => {
            println!(
                "Authentication successful, bytes to return to client: {:?}",
                std::str::from_utf8(&*buffer)
            );
            // TODO: send buffer ?
            conn.send_code(SMTPReplyCode::AuthenticationSucceeded)?;
            anyhow::Ok(true)
        }
        Ok(rsasl::Step::NeedsMore(buffer)) => {
            let reply = format!(
                "334 {}\r\n",
                base64::encode(std::str::from_utf8(&*buffer).unwrap())
            ); // 501 5.5.2
            conn.send(&reply)?;
            Ok(false)
        }
        Err(e) if e.matches(rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR) => {
            panic!("Authentication failed, bad username or password");
        }
        Err(e) => panic!("Authentication errored: {}", e),
    }
}

pub async fn on_authentication<S>(
    conn: &mut Connection<'_, S>,
    rsasl: std::sync::Arc<tokio::sync::Mutex<SaslBackend>>,
    mechanism: Mechanism,
    initial_response: Option<String>,
) -> anyhow::Result<()>
where
    S: std::io::Read + std::io::Write + Send,
{
    // TODO: if mechanism require initial data , but omitted => error
    // TODO: if initial data == "=" ; it mean empty ""

    let mut guard = rsasl.lock().await;
    let mut session = guard.server_start(&String::from(mechanism)).unwrap();

    let mut authenticated = auth_step(
        conn,
        &mut session,
        initial_response.unwrap_or_else(|| "".to_string()),
    )
    .unwrap();
    while !authenticated {
        authenticated = match conn.read(std::time::Duration::from_secs(1)).await {
            Ok(Ok(buffer)) => {
                println!("{}", buffer);
                auth_step(conn, &mut session, buffer).unwrap()
            }
            Ok(Err(e)) => todo!("error {e:?}"),
            Err(e) => todo!("timeout {e}"),
        };
    }

    // TODO: get session property
    Ok(())
}
