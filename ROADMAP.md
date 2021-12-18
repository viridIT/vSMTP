# vSMTP standard edition roadmap

> Release before 1.0 should not be used for production purpose.

For the enhanced edition roadmap, please feel free to contact us at
<https://www.viridit.com/contact>.

## Release 0.7.x

Available in December 2021, these releases focus on:

- Handling network connections, including TLS support
- SMTP compliancy (RFC 5321/5322)
- Interaction with the SMTP transaction
- Scripting language (vSL) and configuration files
- Local delivery using "maildir" (IMAP) protocol
- Application logs

About filtering functionalities :

- All SMTP state : HELO/EHLO, CONNECT, MAIL, RCPT, DATA
- Inline filtering (pre-queue)
- Basic actions (accept, deny, rewrite, etc.)
- User defined quarantine queues and logs
- JSON and RAW exports

> This version only manages incoming mails. For outgoing mail you must use your
> current SMTP server.

## Release 0.8.x

Available early January 2022, these versions main objectives are:

- The delivery process (local/remote, delivery queues, etc.)
- The Post-queue filtering
- Enhance logging

About filtering functionalities :

- MIME compliancy (RFC 2045+) checks
- Actions on MIME headers related to RFC 5322
- Offline filtering stage (post-queue)
- Deliver stage related rules and actions

## Release 0.9.x

These releases will focus on the vSMTP policy server and on performances.

The vSMTP Policy Server (vPS) is a module dedicated to integrating of
third-party software. Thanks to its logic engine it can manage complex filtering
and security rules. In version RC3, vPS will accept delegation trough :

- The SMTP protocol
- A libmilter-like emulator
- Local Unix scripts

It can be called at any stage of a SMTP transaction via the rule engine.

## Production release

Depending on versions 0.7+ fixes and user feedbacks, the production version is
expected for Q2/2022.
