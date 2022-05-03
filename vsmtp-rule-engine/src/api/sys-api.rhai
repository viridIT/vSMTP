/// Here are declared all wrappers around vsl's system functions.
/// the goal is to use each funciton using the '!' syntax to
/// inject `CTX` & `SRV` into the functions without parameters.

/// SMTP transaction handling (rule_state.rs)
fn faccept() { sys::faccept() }
fn accept() { sys::accept() }
fn next() { sys::next() }
fn deny() { sys::deny() }
fn deny(code) { sys::deny(code) }
fn info(code) { sys::info(code) }
fn quarantine(queue) { sys::quarantine(SRV, CTX, queue) }

/// Header handling (headers.rs)
fn has_header(header) { sys::has_header(CTX, header) }
fn get_header(header) { sys::get_header(CTX, header) }
fn add_header(header, value) { sys::add_header(CTX, header, value) }
fn set_header(header, value) { sys::set_header(CTX, header, value) }
// fn add_to(addr) { sys::add_to(CTX, addr) }
// fn remove_to(addr) { sys::remove_to(CTX, addr) }

// Envelop handling (envelop.rs)
fn rewrite_mail_from(new_addr) { sys::rewrite_mail_from(CTX, new_addr) }
fn rewrite_rcpt(old_addr, new_addr) { sys::rewrite_rcpt(CTX, old_addr, new_addr) }
fn add_rcpt(addr) { sys::add_rcpt(CTX, addr) }
fn remove_rcpt(addr) { sys::remove_rcpt(CTX, addr) }
fn bcc(bcc) { sys::bcc(CTX, bcc) }

/// Logging (logging.rs)
fn log(level, message) { sys::log(level, message) }

/// Export messages (exports.rs)
fn write(dir) { sys::write(SRV, CTX, dir) }
fn dump(dir) { sys::dump(SRV, CTX, dir) }

/// Services (services.rs)
fn run_service(service) { sys::run_service(SRV, CTX, service) }

/// Delivery methods (transports.rs)
fn forward(rcpt, forward) { sys::forward(CTX, rcpt, forward) }
fn forward_all(forward) { sys::forward(CTX, forward) }
fn deliver(rcpt) { sys::deliver(CTX, rcpt) }
fn deliver_all() { sys::deliver_all(CTX) }
fn mbox(rcpt) { sys::mbox(CTX, rcpt) }
fn mbox_all() { sys::mbox_all(CTX) }
fn maildir(rcpt) { sys::maildir(CTX, rcpt) }
fn maildir_all() { sys::maildir_all(CTX) }
fn disable_delivery(rcpt) { sys::disable_delivery(CTX, rcpt) }
fn disable_delivery_all() { sys::disable_delivery_all(CTX) }

/// Tools (utils.rs)
fn send_mail(from, to, path, relay) { sys::send_mail(from, to, path, relay) }
fn user_exist(name) { sys::user_exist(name) }
fn hostname() { sys::hostname() }
