// SMTP transaction handling (rule_state.rs)

/// Tell the rule engine to force accept the incomming transaction.
/// This means that all rules following the one `faccept` is called
/// will be ignored.
///
/// Use this return status when you are sure that
/// the incoming client can be trusted.
///
/// # Effective smtp stage
///
/// all of them.
///
/// # Example
/// ```js
/// #{
///     connect: [
///         // Here we imagine that "192.168.1.10" is a trusted source, so we can force accept
///         // any other rules that don't need to be run.
///         rule "check for trusted source" || if client_ip() == "192.168.1.10" { faccept() } else { next() },
///     ],
/// }
///
/// # Module:Status
/// ```
fn faccept() { sys::faccept() }

/// Tell the rule engine to accept the incomming transaction for the current stage.
/// This means that all rules following the one `accept` is called in the current stage
/// will be ignored.
///
/// # Effective smtp stage
///
/// all of them.
///
/// # Example
/// ```js
/// #{
///     connect: [
///         // "ignored checks" will be ignored because the previous rule returned accept.
///         rule "accept" || accept(),
///         rule "ignored checks" || print("this will be ignored.")
///     ],
///
///     mail: [
///         // rule evaluation is resumed in the next stage.
///         rule "resuming rules" || print("we resume rule evaluation here.");
///     ]
/// }
/// ```
///
/// # Module:Status
fn accept() { sys::accept() }

/// Tell the rule engine that a rule succeeded.
///
/// # Effective smtp stage
///
/// all of them.
///
/// # Example
/// ```js
/// #{
///     connect: [
///         // once "go next" is evaluated, the rule engine execute "another rule".
///         rule "go next" || next(),
///         rule "another rule" || print("checking stuff ..."),
///     ],
/// }
/// ```
///
/// # Module:Status
fn next() { sys::next() }

/// Stop rules evaluation and/or send an error code to the client.
/// The code sent is `554 - permanent problems with the remote server`.
///
/// # Effective smtp stage
///
/// all of them.
///
/// # Example
/// ```js
/// #{
///     rcpt: [
///         rule "" || {
///            // The client is denied if a recipient's domain matches satan.org,
///            // this is a blacklist, sort-of.
///            if ctx().rcpt.domain == "satan.org" {
///                deny()
///            } else {
///                next()
///            }
///        },
///     ],
/// }
/// ```
///
/// # Module:Status
fn deny() { sys::deny() }

/// Stop rules evaluation and/or send a custom code to the client.
///
/// # Effective smtp stage
///
/// all of them.
///
/// # Example
/// ```js
/// #{
///     rcpt: [
///         rule "" || {
///            // a custom error code can be used with `deny`.
///            object error_code code = #{ code: 550, enhanced: "", text: "satan.org is not welcome here." };
///
///            // The client is denied if a recipient's domain matches satan.org,
///            // this is a blacklist, sort-of.
///            if ctx().rcpt.domain == "satan.org" {
///                deny(error_code)
///            } else {
///                next()
///            }
///        },
///     ],
/// }
/// ```
///
/// # Module:Status
fn deny(code) { sys::deny(code) }

/// Ask the client to retry to send the current comment by sending an information code.
///
/// # Effective smtp stage
///
/// all of them.
///
/// # Example
/// ```js
/// #{
///     connect: [
///         rule "" || {
///            object info_code code = #{ code: 451, enhanced: "", text: "failed to understand you request, please retry." };
///            info(info_code)
///        },
///     ],
/// }
/// ```
///
/// # Module:Status
fn info(code) { sys::info(code) }

/// Skip all rules until the email is received and place the email in a
/// quarantine queue.
///
/// # Args
///
/// * `queue` - the relative path to the queue where the email will be quarantined.
///
/// # Effective smtp stage
///
/// all of them.
///
/// # Example
/// ```js
/// #{
///     postq: [
///           delegate svc::clamsmtpd "check email for virus" || {
///               // the email is placed in quarantined if a virus is detected by
///               // a service.
///               if has_header("X-Virus-Infected") {
///                 quarantine("virus_queue")
///               } else {
///                 next()
///               }
///           }
///     ],
/// }
/// ```
///
/// # Module:Status
fn quarantine(queue) { sys::quarantine(ctx(), queue) }

/// Checks if the message contains a specific header.
///
/// # Args
///
/// * `header` - the name of the header to search.
///
/// # Effective smtp stage
///
/// `preq` and onwards.
///
/// # Example
/// ```js
/// #{
///     postq: [
///         action "check for VSMTP header" || {
///             if has_header("X-VSMTP") {
///                 log("info", "incoming message could be from another vsmtp server");
///             }
///         }
///     ],
/// }
/// ```
///
/// # Module:Message
fn has_header(header) { sys::has_header(msg(), header) }

/// Get a specific header from the incoming message.
///
/// # Args
///
/// * `header` - the name of the header to get.
///
/// # Return
///
/// * `string` - the header value, or an empty string if the header was not found.
///
/// # Effective smtp stage
///
/// `preq` and onwards.
///
/// # Example
/// ```js
/// #{
///     postq: [
///         action "display VSMTP header" || {
///             print(get_header("X-VSMTP"));
///         }
///     ],
/// }
/// ```
///
/// # Module:Message
fn get_header(header) { sys::get_header(msg(), header) }

/// Append a new header to the message.
///
/// # Args
///
/// * `header` - the name of the header to append.
/// * `value` - the value of the header to append.
///
/// # Effective smtp stage
///
/// All of them. Even tought the email is not received at the current stage,
/// vsmtp stores new headers and will prepend them to the ones received once
/// the `preq` stage is reached.
///
/// # Example
/// ```js
/// #{
///     postq: [
///         action "append a header" || {
///             append_header("X-JOHN", "received by john's server.");
///         }
///     ],
/// }
/// ```
///
/// # Module:Message
fn append_header(header, value) { sys::append_header(msg(), header, value) }

/// Prepend a new header to the message.
///
/// # Args
///
/// * `header` - the name of the header to prepend.
/// * `value` - the value of the header to prepend.
///
/// # Effective smtp stage
///
/// All of them. Even tought the email is not received at the current stage,
/// vsmtp stores new headers and will prepend them to the ones received once
/// the `preq` stage is reached.
///
/// # Example
/// ```js
/// #{
///     postq: [
///         action "prepend a header" || {
///             prepend_header("X-JOHN", "received by john's server.");
///         }
///     ],
/// }
/// ```
///
/// # Module:Message
fn prepend_header(header, value) { sys::prepend_header(msg(), header, value) }

/// Replace an existing header value by a new value, or append a new header
/// to the message.
///
/// # Args
///
/// * `header` - the name of the header to set or add.
/// * `value` - the value of the header to set or add.
///
/// # Effective smtp stage
///
/// All of them. Even tought the email is not received at the current stage,
/// vsmtp stores new headers and will prepend them to the ones received once
/// the `preq` stage is reached.
///
/// Be aware that if you want to set a header value from the original message,
/// you must use `set_header` in the `preq` stage and onwards.
///
/// # Example
/// ```js
/// #{
///     postq: [
///         action "update subject" || {
///             let subject = get_header("Subject");
///             set_header("Subject", `${subject} (analysed by vsmtp)`);
///         }
///     ],
/// }
/// ```
///
/// # Module:Message
fn set_header(header, value) { sys::set_header(msg(), header, value) }

/// Add a recipient to the `To` header of the message.
///
/// # Args
///
/// * `addr` - the recipient address to add to the `To` header.
///
/// # Effective smtp stage
///
/// `preq` and onwards.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "update recipients" || add_rcpt_message("john.doe@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Message
fn add_rcpt_message(addr) { sys::add_rcpt_message(msg(), addr) }

/// Remove a recipient from the `To` header of the message.
///
/// # Args
///
/// * `addr` - the recipient to remove to the `To` header.
///
/// # Effective smtp stage
///
/// `preq` and onwards.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "update recipients" || remove_rcpt_message("john.doe@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Message
fn remove_rcpt_message(addr) { sys::remove_rcpt_message(msg(), addr) }

/// Replace a recipient by an other in the `To` header of the message.
///
/// # Args
///
/// * `old_addr` - the recipient to replace.
/// * `new_addr` - the new address to use when replacing `old_addr`.
///
/// # Effective smtp stage
///
/// `preq` and onwards.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "rewrite recipient" || rewrite_rcpt_message("john.doe@example.com", "john-mta@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Message
fn rewrite_rcpt_message(old_addr, new_addr) {
    sys::rewrite_rcpt_message(msg(), old_addr, new_addr)
}

/// Change the sender's address in the `From` header of the message.
///
/// # Args
///
/// * `new_addr` - the new sender address to set.
///
/// # Effective smtp stage
///
/// `preq` and onwards.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "replace sender" || rewrite_mail_from_message("john.server@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Message
fn rewrite_mail_from_message(new_addr) { sys::rewrite_mail_from_message(msg(), new_addr) }

/// Get the address of the client.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Return
///
/// * `string` - the client's address with the `ip:port` format.
///
/// # Example
/// ```js
/// #{
///     connect: [
///        action "log info" || log("info", `${client_address()}`),
///     ]
/// }
/// ```
///
/// # Module:Connection
fn client_address() { ctx().client_address }

/// Get the ip address of the client.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Return
///
/// * `string` - the client's ip address.
///
/// # Example
/// ```js
/// #{
///     connect: [
///        action "log info" || log("info", `${client_ip()}`),
///     ]
/// }
/// ```
///
/// # Module:Connection
fn client_ip() { ctx().client_ip }

/// Get the ip port of the client.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Return
///
/// * `int` - the client's port.
///
/// # Example
/// ```js
/// #{
///     connect: [
///        action "log info" || log("info", `${client_port()}`),
///     ]
/// }
/// ```
///
/// # Module:Connection
fn client_port() { ctx().client_port }

/// Get the full server address.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Return
///
/// * `string` - the server's address with the `ip:port` format.
///
/// # Example
/// ```js
/// #{
///     connect: [
///        action "log info" || log("info", `${server_address()}`),
///     ]
/// }
/// ```
///
/// # Module:Connection
fn server_address() { ctx().server_address }

/// Get the server's ip.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Return
///
/// * `string` - the server's ip.
///
/// # Example
/// ```js
/// #{
///     connect: [
///        action "log info" || log("info", `${server_ip()}`),
///     ]
/// }
/// ```
///
/// # Module:Connection
fn server_ip() { ctx().server_ip }

/// Get the server's port.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Return
///
/// * `string` - the server's port.
///
/// # Example
/// ```js
/// #{
///     connect: [
///        action "log info" || log("info", `${server_port()}`),
///     ]
/// }
/// ```
///
/// # Module:Connection
fn server_port() { ctx().server_port }

/// Get a the timestamp of the client's connection time.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Return
///
/// * `timestamp` - the connexion timestamp of the client.
///
/// # Example
/// ```js
/// #{
///     connect: [
///        action "log info" || log("info", `${connection_timestamp()}`),
///     ]
/// }
/// ```
///
/// # Module:Connection
fn connection_timestamp() { ctx().connection_timestamp }

/// Get the name of the server.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Return
///
/// * `string` - the name of the server.
///
/// # Example
/// ```js
/// #{
///     connect: [
///        action "log info" || log("info", `${server_name()}`),
///     ]
/// }
/// ```
///
/// # Module:Connection
fn server_name() { ctx().server_name }

/// Check if the client's connexion was secure.
///
/// # Effective smtp stage
///
/// `authenticate` only.
///
/// # Return
///
/// * `bool` - true if the client securly connected with the auth protocol, false otherwise.
///
/// # Example
/// ```js
/// #{
///     authenticate: [
///        action "log info" || log("info", `${is_secured()}`),
///     ]
/// }
/// ```
///
/// # Module:Auth
fn is_secured() { ctx().is_secured }

/// Check if the client is authenticated.
///
/// # Effective smtp stage
///
/// `authenticate` only.
///
/// # Return
///
/// * `bool` - true if the client succedded to authenticate itself, false otherwise.
///
/// # Example
/// ```js
/// #{
///     authenticate: [
///        action "log info" || log("info", `${is_authenticated()}`),
///     ]
/// }
/// ```
///
/// # Module:Auth
fn is_authenticated() { ctx().is_authenticated }

/// Get authentication credentials from the client.
///
/// # Effective smtp stage
///
/// `authenticate` only.
///
/// # Return
///
/// * `Credentials` - the credentials of the client.
///
/// # Example
/// ```js
/// #{
///     authenticate: [
///        action "log info" || log("info", `${auth()}`),
///     ]
/// }
/// ```
///
/// # Module:Auth
fn auth() { ctx().auth }

/// Get the value of the `HELO/EHLO` command sent by the client.
///
/// # Effective smtp stage
///
/// `helo` and onwards.
///
/// # Return
///
/// * `string` - the value of the `HELO/EHLO` command.
///
/// # Example
/// ```js
/// #{
///     helo: [
///        action "log info" || log("info", `${helo()}`),
///     ]
/// }
/// ```
///
/// # Module:Transaction
fn helo() { ctx().helo }

/// Get the value of the `MAIL FROM` command sent by the client.
///
/// # Effective smtp stage
///
/// `mail` and onwards.
///
/// # Return
///
/// * `address` - the sender address.
///
/// # Example
/// ```js
/// #{
///     helo: [
///        action "log info" || log("info", `${mail_from()}`),
///     ]
/// }
/// ```
///
/// # Module:Transaction
fn mail_from() { ctx().mail_from }

/// Get the value of the current `RCPT TO` command sent by the client.
///
/// # Effective smtp stage
///
/// `rcpt` and onwards. Please note that `rcpt()` will always return
/// the last recipient received in stages after the `rcpt` stage
///
/// # Return
///
/// * `address` - the address of the received recipient.
///
/// # Example
/// ```js
/// #{
///     rcpt: [
///        action "log recipients" || log("info", `new recipient: ${rcpt()}`),
///     ]
/// }
/// ```
///
/// # Module:Transaction
fn rcpt() { ctx().rcpt }

/// Get the list of recipients received by the client.
///
/// # Effective smtp stage
///
/// `rcpt` and onwards. Note that you will not have all recipients received
/// all at once in the `rcpt` stage. It is better to use this function
/// in the later stages.
///
/// # Return
///
/// * `Array of addresses` - the list containing all recipients.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "log recipients" || log("info", `all recipients: ${rcpt_list()}`),
///     ]
/// }
/// ```
///
/// # Module:Transaction
fn rcpt_list() { ctx().rcpt_list }

/// Get the time of reception of the email.
///
/// # Effective smtp stage
///
/// `preq` and onwards.
///
/// # Return
///
/// * `string` - the timestamp.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "receiving the email" || log("info", `time of reception: ${mail_timestamp()}`),
///     ]
/// }
/// ```
///
/// # Module:Transaction
fn mail_timestamp() { ctx().mail_timestamp }

/// Get the unique id of the received message.
///
/// # Effective smtp stage
///
/// `preq` and onwards.
///
/// # Return
///
/// * `string` - the message id.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "message received" || log("info", `message id: ${message_id()}`),
///     ]
/// }
/// ```
///
/// # Module:Transaction
fn message_id() { ctx().message_id }

/// Rewrite the sender received from the `MAIL FROM` command.
///
/// # Args
///
/// * `new_addr` - the new sender address to set.
///
/// # Effective smtp stage
///
/// `mail` and onwards.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "rewrite envelop" || rewrite_mail_from_envelop("unknown@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Envelop
fn rewrite_mail_from_envelop(new_addr) { sys::rewrite_mail_from_envelop(ctx(), new_addr) }

/// Replace a recipient received by a `RCPT TO` command.
///
/// # Args
///
/// * `old_addr` - the recipient to replace.
/// * `new_addr` - the new address to use when replacing `old_addr`.
///
/// # Effective smtp stage
///
/// `rcpt` and onwards.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "rewrite envelop" || rewrite_rcpt_envelop("john.doe@example.com", "john.main@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Envelop
fn rewrite_rcpt_envelop(old_addr, new_addr) { sys::rewrite_rcpt_envelop(ctx(), old_addr, new_addr) }

/// Add a new recipient to the envelop. Note that this does not add
/// the recipient to the `To` header. Use `add_rcpt_message` for that.
///
/// # Args
///
/// * `rcpt` - the new recipient to add.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     connect: [
///        // always deliver a copy of the message to "john.doe@example.com".
///        action "rewrite envelop" || add_rcpt_envelop("john.doe@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Envelop
fn add_rcpt_envelop(rcpt) { sys::add_rcpt_envelop(ctx(), rcpt) }

/// Remove a recipient from the envelop. Note that this does not remove
/// the recipient from the `To` header. Use `remove_rcpt_message` for that.
///
/// # Args
///
/// * `rcpt` - the recipient to remove.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        // never deliver to "john.doe@example.com".
///        action "rewrite envelop" || remove_rcpt_envelop("john.doe@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Envelop
fn remove_rcpt_envelop(rcpt) { sys::remove_rcpt_envelop(ctx(), rcpt) }

/// Add a recipient as a blind carbon copy. The equivalent of `add_rcpt_envelop`.
///
/// # Args
///
/// * `rcpt` - the recipient to add as a blind carbon copy.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     connect: [
///        // set "john.doe@example.com" as a blind carbon copy.
///        action "bcc" || bcc("john.doe@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Message
fn bcc(rcpt) { add_rcpt_envelop(rcpt) }

/// Rewrite the value of the `MAIL FROM` command has well has
/// the `From` header.
///
/// # Args
///
/// * `new_addr` - the new sender address to set.
///
/// # Effective smtp stage
///
/// `preq` and onwards.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "rewrite sender" || rewrite_mail_from("john.doe@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Envelop
fn rewrite_mail_from(new_addr) {
    sys::rewrite_mail_from_envelop(ctx(), new_addr);
    sys::rewrite_mail_from_message(msg(), new_addr)
}

/// Log information to stdout in `nodaemon` mode or to a file.
///
/// # Args
///
/// * `level` - the level of the message, can be "trace", "debug", "info", "warn" or "error".
/// * `message` - the message to log.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "log info" || log("info", "this is an informational log."),
///     ]
/// }
/// ```
///
/// # Module:Utils
fn log(level, message) { sys::log(level, message) }

/// Export the current raw message to a file as an `eml` file.
/// The message id of the email is used to name the file.
///
/// # Args
///
/// * `dir` - the directory where to store the email. Relative to the
/// application path.
///
/// # Effective smtp stage
///
/// `preq` and onwards.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "write to file" || write("archives"),
///     ]
/// }
/// ```
///
/// # Module:Utils
fn write(dir) { sys::write(srv(), ctx(), msg(), dir) }

/// Export the current message and the envelop to a file as a `json` file.
/// The message id of the email is used to name the file.
///
/// # Args
///
/// * `dir` - the directory where to store the data. Relative to the
/// application path.
///
/// # Effective smtp stage
///
/// `preq` and onwards.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "dump email" || dump("metadatas"),
///     ]
/// }
/// ```
///
/// # Module:Utils
fn dump(dir) { sys::dump(srv(), ctx(), dir) }

/// Set the delivery method to forwarding for a single recipient.
/// After all rules are evaluated, forwarding will be used to deliver
/// the email to the recipient.
///
/// # Args
///
/// * `rcpt` - the recipient to apply the method to.
/// * `target` - the target to forward the email to.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     delivery: [
///        action "setup forwarding" || forward("john.doe@example.com", "mta-john.example.com"),
///     ]
/// }
/// ```
///
/// # Module:Delivery
fn forward(rcpt, target) { sys::forward(ctx(), rcpt, target) }

/// Set the delivery method to forwarding for all recipients.
/// After all rules are evaluated, forwarding will be used to deliver
/// the email.
///
/// # Args
///
/// * `target` - the target to forward the email to.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     delivery: [
///        action "setup forwarding" || forward_all("mta-john.example.com"),
///     ]
/// }
/// ```
///
/// # Module:Delivery
fn forward_all(target) { sys::forward_all(ctx(), target) }

/// Set the delivery method to deliver for a single recipient.
/// After all rules are evaluated, the email will be sent
/// to the recipient using the domain of its address.
///
/// # Args
///
/// * `rcpt` - the recipient to apply the method to.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     delivery: [
///        action "setup delivery" || deliver("john.doe@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Delivery
fn deliver(rcpt) { sys::deliver(ctx(), rcpt) }

/// Set the delivery method to deliver for all recipients.
/// After all rules are evaluated, the email will be sent
/// to all recipients using the domain of their respective address.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     delivery: [
///        action "setup delivery" || deliver_all(),
///     ]
/// }
/// ```
///
/// # Module:Delivery
fn deliver_all() { sys::deliver_all(ctx()) }

/// Set the delivery method to mbox for a recipient.
/// After all rules are evaluated, the email will be stored
/// localy in the mail box of the recipient if it exists on the server.
///
/// # Args
///
/// * `rcpt` - the recipient to apply the method to.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     delivery: [
///        action "setup mbox" || mbox("john.doe@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Delivery
fn mbox(rcpt) { sys::mbox(ctx(), rcpt) }

/// Set the delivery method to mbox for all recipients.
/// After all rules are evaluated, the email will be stored
/// localy in the mail box of all recipients if they exists on the server.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     delivery: [
///        action "setup mbox" || mbox_all(),
///     ]
/// }
/// ```
///
/// # Module:Delivery
fn mbox_all() { sys::mbox_all(ctx()) }

/// Set the delivery method to maildir for a recipient.
/// After all rules are evaluated, the email will be stored
/// localy in the `~/Maildir/new/` folder of the recipient's user if it exists on the server.
///
/// # Args
///
/// * `rcpt` - the recipient to apply the method to.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     delivery: [
///        action "setup maildir" || maildir("john.doe@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Delivery
fn maildir(rcpt) { sys::maildir(ctx(), rcpt) }

/// Set the delivery method to maildir for all recipients.
/// After all rules are evaluated, the email will be stored
/// localy in each `~/Maildir/new` folder of they respective recipient
/// if they exists on the server.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     delivery: [
///        action "setup mbox" || mbox_all(),
///     ]
/// }
/// ```
///
/// # Module:Delivery
fn maildir_all() { sys::maildir_all(ctx()) }

/// Disable the delivery for a single recipient.
///
/// # Args
///
/// * `rcpt` - the recipient to apply the method to.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     delivery: [
///        action "disable delivery" || disable_delivery("john.doe@example.com"),
///     ]
/// }
/// ```
///
/// # Module:Delivery
fn disable_delivery(rcpt) { sys::disable_delivery(ctx(), rcpt) }

/// Disable delivery for all single recipients.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     delivery: [
///        action "disable delivery" || disable_delivery_all(),
///     ]
/// }
/// ```
///
/// # Module:Delivery
fn disable_delivery_all() { sys::disable_delivery_all(ctx()) }

//
// fn send_mail(from, to, path, relay) { sys::send_mail(from, to, path, relay) }

/// Check if a user exists on this server.
///
/// # Args
///
/// * `name` - the name of the user.
///
/// # Return
///
/// * `bool` - true if the user exists, false otherwise.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     rcpt: [
///        action "check for local user" || {
///            if user_exist(rcpt().local_part) {
///                log("debug", `${rcpt().local_part} exists on disk.`);
///            }
///        }
///     ]
/// }
/// ```
///
/// # Module:Utils
fn user_exist(name) { sys::user_exist(name) }

/// Get the hostname of this machine.
///
/// # Return
///
/// * `string` - the host name of the machine.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "append info header" || {
///             append_header("X-VSMTP", `email received by ${hostname()}.`);
///        }
///     ]
/// }
/// ```
///
/// # Module:Utils
fn hostname() { sys::hostname() }

/// Get the current date.
///
/// # Return
///
/// * `string` - the current date.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "append info header" || {
///             append_header("X-VSMTP", `email received by ${hostname()} the ${date()}.`);
///        }
///     ]
/// }
/// ```
///
/// # Module:Utils
fn date() { sys::date() }

/// Get the current time.
///
/// # Return
///
/// * `string` - the current time.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// #{
///     preq: [
///        action "append info header" || {
///             append_header("X-VSMTP", `email received by ${hostname()} the ${date()} at ${time()}.`);
///        }
///     ]
/// }
/// ```
///
/// # Module:Utils
fn time() { sys::time() }

/// Get the value of a key in a database.
///
/// # Args
///
/// * `key` - the key to query.
///
/// # Return
///
/// * `Array of records` - an array containing the results.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// import "services" as svc;
///
/// #{
///     mail: [
///        action "fetch database" || {
///             let records = svc::my_database.get(mail_from());
///
///             if records.len() == 0 {
///                 log("debug", `${mail_from()} is not in my database`);
///             } else {
///                 log("debug", `${mail_from()} found in my database`);
///             }
///        }
///     ]
/// }
/// ```
///
/// # Module:Services
fn get(key) { this.db_query(key.to_string()) }

/// Set a record into a database.
///
/// # Args
///
/// * `record` - the record to set.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// import "services" as svc;
///
/// #{
///     mail: [
///        action "set sender in database" || {
///             svc::my_database.set(mail_from());
///        }
///     ]
/// }
/// ```
///
/// # Module:Services
fn set(record) { this.db_add(record) }

/// Remove a record from a database.
///
/// # Args
///
/// * `key` - the key to remove.
///
/// # Effective smtp stage
///
/// All of them.
///
/// # Example
/// ```js
/// import "services" as svc;
///
/// #{
///     mail: [
///        action "remove sender from database" || {
///             svc::my_database.rm(mail_from());
///        }
///     ]
/// }
/// ```
///
/// # Module:Services
fn rm(key) { this.db_rm(key.to_string()) }
