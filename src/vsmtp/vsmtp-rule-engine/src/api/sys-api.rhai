// SMTP transaction handling (rule_state.rs)

/// Tell the rule engine to force accept the incomming transaction.
/// This means that all rules following the one `faccept` is called
/// will be ignored. Use this return status when you are sure that
/// the incoming client can be trusted.
///
/// # Example
/// ```
/// #{
///     connect: [
///         // Here we imagine that "192.168.1.10" is a trusted source, so we can force accept
///         // any other rules that don't need to be run.
///         rule "is this my server ?" || if client_ip() == "192.168.1.10" { faccept() } else { next() },
///     ],
/// };
/// ```
fn faccept() { sys::faccept() }
fn accept() { sys::accept() }
fn next() { sys::next() }
fn deny() { sys::deny() }
fn deny(code) { sys::deny(code) }
fn info(code) { sys::info(code) }
fn quarantine(queue) { sys::quarantine(ctx(), queue) }

/// Message handling (message.rs)
fn has_header(header) { sys::has_header(msg(), header) }
fn get_header(header) { sys::get_header(msg(), header) }
fn append_header(header, value) { sys::append_header(msg(), header, value) }
fn prepend_header(header, value) { sys::prepend_header(msg(), header, value) }
fn set_header(header, value) { sys::set_header(msg(), header, value) }
fn add_to_message(addr) { sys::add_to_message(msg(), addr) }
fn rewrite_mail_from_message(new_addr) { sys::rewrite_mail_from_message(msg(), new_addr) }
fn remove_to_message(addr) { sys::remove_to_message(msg(), addr) }
fn rewrite_to_message(old_addr, new_addr) {
    sys::rewrite_to_message(msg(), old_addr, new_addr)
}

// Context handling (mail_context.rs)
// -- Getters
fn client_address() { ctx().client_address }
fn client_ip() { ctx().client_ip }
fn client_port() { ctx().client_port }
fn server_address() { ctx().server_address }
fn server_ip() { ctx().server_ip }
fn server_port() { ctx().server_port }
fn connection_timestamp() { ctx().connection_timestamp }
fn server_name() { ctx().server_name }
fn is_secured() { ctx().is_secured }
fn is_authenticated() { ctx().is_authenticated }
fn auth() { ctx().auth }
fn helo() { ctx().helo }
fn mail_from() { ctx().mail_from }
fn rcpt_list() { ctx().rcpt_list }
fn rcpt() { ctx().rcpt }
fn mail_timestamp() { ctx().mail_timestamp }
fn message_id() { ctx().message_id }

// -- Envelop
fn rewrite_mail_from(new_addr) {
    sys::rewrite_mail_from_context(ctx(), new_addr);
    sys::rewrite_mail_from_message(msg(), new_addr)
}
fn rewrite_mail_from_context(new_addr) { sys::rewrite_mail_from_context(ctx(), new_addr) }
fn rewrite_rcpt_context(old_addr, new_addr) { sys::rewrite_rcpt_context(ctx(), old_addr, new_addr) }
fn add_rcpt_context(addr) { sys::add_rcpt_context(ctx(), addr) }
fn remove_rcpt_context(addr) { sys::remove_rcpt_context(ctx(), addr) }
fn bcc(bcc) { sys::bcc(ctx(), bcc) }

/// Logging (logging.rs)
fn log(level, message) { sys::log(level, message) }

/// Export messages (exports.rs)
fn write(dir) { sys::write(srv(), ctx(), msg(), dir) }
fn dump(dir) { sys::dump(srv(), ctx(), dir) }

/// Delivery methods (transports.rs)
fn forward(rcpt, forward) { sys::forward(ctx(), rcpt, forward) }
fn forward_all(forward) { sys::forward_all(ctx(), forward) }
fn deliver(rcpt) { sys::deliver(ctx(), rcpt) }
fn deliver_all() { sys::deliver_all(ctx()) }
fn mbox(rcpt) { sys::mbox(ctx(), rcpt) }
fn mbox_all() { sys::mbox_all(ctx()) }
fn maildir(rcpt) { sys::maildir(ctx(), rcpt) }
fn maildir_all() { sys::maildir_all(ctx()) }
fn disable_delivery(rcpt) { sys::disable_delivery(ctx(), rcpt) }
fn disable_delivery_all() { sys::disable_delivery_all(ctx()) }

/// Tools (utils.rs)
fn send_mail(from, to, path, relay) { sys::send_mail(from, to, path, relay) }
fn user_exist(name) { sys::user_exist(name) }
fn hostname() { sys::hostname() }
fn date() { sys::date() }
fn time() { sys::time() }

/// Databases (services.rs)
fn get(key) { this.db_query(key.to_string()) }
fn set(record) { this.db_add(record) }
fn rm(key) { this.db_rm(key.to_string()) }
