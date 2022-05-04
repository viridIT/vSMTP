/// here are declared all wrappers around vsl's functions declared in rhai.

/// black boxed functions used to access the engine state.
fn ctx() { CTX }
fn srv() { SRV }

/// utils.rhai
fn in_domain(rcpt) { __in_domain(rcpt) }
