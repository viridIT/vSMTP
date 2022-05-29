/// Here are declared all wrappers around vsl's system functions.
/// the goal is to use each funciton using the '!' syntax to
/// inject `ctx()` & `srv()` into the functions without parameters.

/// SMTP transaction handling (rule_state.rs)
fn faccept() { sys::faccept() }
fn accept() { sys::accept() }
fn next() { sys::next() }
fn deny() { sys::deny() }
fn deny(code) { sys::deny(code) }
fn info(code) { sys::info(code) }
fn quarantine(queue) { sys::quarantine(ctx(), queue) }

/// Header handling (headers.rs)
fn has_header(header) { sys::has_header(msg(), header) }
fn get_header(header) { sys::get_header(msg(), header) }
fn add_header(header, value) { sys::add_header(msg(), header, value) }
fn set_header(header, value) { sys::set_header(msg(), header, value) }
fn add_to(addr) { sys::add_to(msg(), addr) }
fn remove_to(addr) { sys::remove_to(msg(), addr) }
fn rewrite_to(old_addr, new_addr) {
    sys::rewrite_to_message(msg(), old_addr, new_addr)
}

// Envelop handling (envelop.rs)
fn rewrite_mail_from(new_addr) {
    sys::rewrite_mail_from_context(ctx(), new_addr);
    sys::rewrite_mail_from_message(msg(), new_addr)
}
fn rewrite_rcpt(old_addr, new_addr) { sys::rewrite_rcpt(ctx(), old_addr, new_addr) }
fn add_rcpt(addr) { sys::add_rcpt(ctx(), addr) }
fn remove_rcpt(addr) { sys::remove_rcpt(ctx(), addr) }
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
