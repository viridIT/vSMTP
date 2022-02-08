/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
**/
use rhai::plugin::*;
#[allow(dead_code)]
#[export_module]
pub mod email {

    use crate::{config::log_channel::RULES, rules::address::Address, smtp::mail::MailContext};
    use std::sync::{Arc, RwLock};

    #[rhai_fn(get = "client_addr", return_raw)]
    pub fn client_addr(
        this: &mut Arc<RwLock<MailContext>>,
    ) -> Result<std::net::SocketAddr, Box<EvalAltResult>> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .client_addr)
    }

    #[rhai_fn(get = "helo", return_raw)]
    pub fn helo(this: &mut Arc<RwLock<MailContext>>) -> Result<String, Box<EvalAltResult>> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .helo
            .clone())
    }

    #[rhai_fn(get = "mail_from", return_raw)]
    pub fn mail_from(this: &mut Arc<RwLock<MailContext>>) -> Result<Address, Box<EvalAltResult>> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .mail_from
            .clone())
    }

    #[rhai_fn(get = "rcpt", return_raw)]
    pub fn rcpt(
        this: &mut Arc<RwLock<MailContext>>,
    ) -> Result<std::collections::HashSet<Address>, Box<EvalAltResult>> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .clone())
    }

    #[rhai_fn(return_raw)]
    pub fn to_string(this: &mut Arc<RwLock<MailContext>>) -> Result<String, Box<EvalAltResult>> {
        Ok(format!(
            "{:?}",
            this.read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
        ))
    }

    #[rhai_fn(return_raw)]
    pub fn to_debug(this: &mut Arc<RwLock<MailContext>>) -> Result<String, Box<EvalAltResult>> {
        Ok(format!(
            "{:#?}",
            this.read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
        ))
    }

    /// checks if the object exists and check if it matches against the connect value.
    pub fn is_connect(ctx: &mut Arc<RwLock<MailContext>>, ip: &str) -> bool {
        match <std::net::IpAddr as std::str::FromStr>::from_str(ip) {
            Ok(ip) => ip == ctx.read().unwrap().client_addr.ip(),
            Err(_) => {
                log::error!(
                    target: RULES,
                    "tried to convert '{}' to an ip but conversion failed.",
                    ip
                );
                false
            }
        }

        // match acquire_engine().objects.read().unwrap().get(object) {
        //     Some(object) => internal_is_connect(&connect.ip(), object),
        //     None => match <std::net::IpAddr as std::str::FromStr>::from_str(object) {
        //         Ok(ip) => ip == connect.ip(),
        //         Err(_) => {
        //             log::error!(
        //                     target: RULES,
        //                     "tried to convert '{}' to an ip because it is not a object, but conversion failed.",
        //                     object
        //                 );
        //             false
        //         }
        //     },
        // }
    }

    //     // TODO: the following functions could be refactored as one.
    //     /// checks if the object exists and check if it matches against the helo value.
    //     pub fn __is_helo(helo: &str, object: &str) -> bool {
    //         match acquire_engine().objects.read().unwrap().get(object) {
    //             Some(object) => internal_is_helo(helo, object),
    //             _ => object == helo,
    //         }
    //     }

    //     /// checks if the object exists and check if it matches against the mail value.
    //     pub fn __is_mail(mail: &mut Address, object: &str) -> bool {
    //         match acquire_engine().objects.read().unwrap().get(object) {
    //             Some(object) => internal_is_mail(mail, object),
    //             // TODO: allow for user / domain search with a string.
    //             _ => object == mail.full(),
    //         }
    //     }

    //     /// checks if the object exists and check if it matches against the rcpt value.
    //     pub fn __is_rcpt(rcpt: &mut Address, object: &str) -> bool {
    //         match acquire_engine().objects.read().unwrap().get(object) {
    //             Some(object) => internal_is_rcpt(rcpt, object),
    //             // TODO: allow for user / domain search with a string.
    //             _ => rcpt.full() == object,
    //         }
    //     }

    //     /// check if the given object matches one of the incoming recipients.
    //     pub fn __contains_rcpt(rcpts: &mut HashSet<Address>, object: &str) -> bool {
    //         match acquire_engine().objects.read().unwrap().get(object) {
    //             Some(object) => rcpts.iter().any(|rcpt| internal_is_rcpt(rcpt, object)),
    //             // TODO: allow for user / domain search with a string.
    //             _ => rcpts.iter().any(|rcpt| rcpt.full() == object),
    //         }
    //     }

    //     /// checks if the given user exists on the system.
    //     pub fn __user_exists(object: &str) -> bool {
    //         match acquire_engine().objects.read().unwrap().get(object) {
    //             Some(object) => internal_user_exists(object),
    //             _ => internal_user_exists(&Object::Var(object.to_string())),
    //         }
    //     }

    #[rhai_fn(return_raw)]
    pub fn use_resolver(
        this: &mut Arc<RwLock<MailContext>>,
        resolver: String,
    ) -> Result<(), Box<EvalAltResult>> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .metadata
            .as_mut()
            .ok_or_else::<Box<EvalAltResult>, _>(|| {
                "failed to use default resolver: email metadata are unavailable".into()
            })?
            .resolver = resolver;

        Ok(())
    }
}

// // NOTE: the following functions use pub(super) because they need to be exposed for tests.
// // FIXME: find a way to hide the following function to the parent scope.
// /// checks recursively if the current connect value is matching the object's value.
// pub(super) fn internal_is_connect(connect: &std::net::IpAddr, object: &Object) -> bool {
//     match (&connect, object) {
//         (std::net::IpAddr::V4(connect), Object::Ip4(ip)) => *ip == *connect,
//         (std::net::IpAddr::V6(connect), Object::Ip6(ip)) => *ip == *connect,
//         (std::net::IpAddr::V4(connect), Object::Rg4(range)) => range.contains(connect),
//         (std::net::IpAddr::V6(connect), Object::Rg6(range)) => range.contains(connect),
//         // NOTE: is there a way to get a &str instead of a String here ?
//         (connect, Object::Regex(re)) => re.is_match(connect.to_string().as_str()),
//         (connect, Object::File(content)) => content
//             .iter()
//             .any(|object| internal_is_connect(connect, object)),
//         (connect, Object::Group(group)) => group
//             .iter()
//             .any(|object| internal_is_connect(connect, object)),
//         _ => false,
//     }
// }

// /// checks recursively if the current helo value is matching the object's value.
// pub(super) fn internal_is_helo(helo: &str, object: &Object) -> bool {
//     match object {
//         Object::Fqdn(fqdn) => *fqdn == helo,
//         Object::Regex(re) => re.is_match(helo),
//         Object::File(content) => content.iter().any(|object| internal_is_helo(helo, object)),
//         Object::Group(group) => group.iter().any(|object| internal_is_helo(helo, object)),
//         _ => false,
//     }
// }

// /// checks recursively if the current mail value is matching the object's value.
// pub(super) fn internal_is_mail(mail: &Address, object: &Object) -> bool {
//     match object {
//         Object::Var(user) => mail.local_part() == user,
//         Object::Fqdn(domain) => mail.domain() == domain,
//         Object::Address(addr) => addr == mail,
//         Object::Regex(re) => re.is_match(mail.full()),
//         Object::File(content) => content.iter().any(|object| internal_is_mail(mail, object)),
//         Object::Group(group) => group.iter().any(|object| internal_is_mail(mail, object)),
//         _ => false,
//     }
// }

// /// checks recursively if the current rcpt value is matching the object's value.
// pub(super) fn internal_is_rcpt(rcpt: &Address, object: &Object) -> bool {
//     match object {
//         Object::Var(user) => rcpt.local_part() == user,
//         Object::Fqdn(domain) => rcpt.domain() == domain,
//         Object::Address(addr) => rcpt == addr,
//         Object::Regex(re) => re.is_match(rcpt.full()),
//         Object::File(content) => content.iter().any(|object| internal_is_rcpt(rcpt, object)),
//         Object::Group(group) => group.iter().any(|object| internal_is_rcpt(rcpt, object)),
//         _ => false,
//     }
// }

// /// checks recursively if the/all user(s) exists on the system.
// pub(super) fn internal_user_exists(user: &Object) -> bool {
//     match user {
//         Object::Var(user) => user_exists(user),
//         Object::Address(addr) => user_exists(addr.local_part()),
//         Object::File(content) | Object::Group(content) => content.iter().all(internal_user_exists),
//         _ => false,
//     }
// }
