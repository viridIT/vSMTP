/// here are declared all wrappers around vsl's functions.
/// the goal is to use each funciton using the '!' syntax to
/// inject `ctx` & `srv` into the functions without parameters.

/// utils.rhai
fn get_domain(rcpt) { __get_domain(rcpt) }
