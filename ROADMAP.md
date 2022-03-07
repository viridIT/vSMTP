# vSMTP standard edition roadmap

> Release before 1.0 should not be used for production purpose.
For the enhanced edition roadmap, please feel free to contact us at
<https://www.viridit.com/contact>.

## Release 0.7.x

Available from December 2021, this release focuses on:

- Handling network connections, including TLS support.
- SMTP compliancy (RFC 5321/5322).
- Interaction with the SMTP transaction.
- Scripting language (vSL) and configuration files.
- Local delivery using "maildir" (IMAP) protocol.
- Application logs.

About filtering functionalities :

- All SMTP state : HELO/EHLO, CONNECT, MAIL, RCPT, DATA.
- Inline filtering (pre-queue).
- Basic actions like accept, deny, rewrite, etc.
- User defined quarantine queues and logs.
- JSON and RAW exports.

> This version only manages incoming mails. An IMAP server is required. For
> outgoing mail you must use your current SMTP server.

## Release 0.8.x

Available from mid January 2022, the main features are:

- The delivery process and the related queues for local and remote mails.
- The Post-queue filtering.

About filtering functionalities :

- MIME compliancy (RFC 2045+) checks.
- Actions on MIME headers related to RFC 5322.
- Offline filtering stage (post-queue).
- Deliver stage related rules and actions.

## Release 0.9.x : current version

Due to several constraints the vSMTP Policy Server (vPS) module dedicated to the integration of third-party software has been postponed to versions 0.10.x.

The 0.9.x releases focus on:

- TLS integration.
- vSL grammar and syntax.
- Refactoring of TOML tables/fields.
- Local Unix services.

About system integration and security:

- Daemon startup mode and drop of privileges.
- Split of system and application logs.

About filtering features:

- Headers manipulation.
- Bcc() action.

## Release 0.10.x : available in april 2022

These versions will be the first pre-production releases, focusing on vSMTP policy server and performances.

The vSMTP Policy Server (vPS) is a module dedicated to the integration of
third-party software. Thanks to its logic engine it can manage complex filtering
and security rules. In version 0.10.x, vPS will accept delegation trough:

- The SMTP protocol.
- Postfix SMTP access policy delegation support.
- Local Unix scripts.

It can be called at any stage of a SMTP transaction via the rule engine.

This version will also include:

- Folders restrictions for user-defined quarantines.
- system & application logs.
- a new `server` vsl api that will enable interaction with services and server configuration in rules.

## Production release

Depending on versions fixes and user feedbacks, the production version is
expected for end of Q2/2022.
