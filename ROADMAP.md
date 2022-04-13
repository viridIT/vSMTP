# vSMTP standard edition roadmap

## Production release

Depending on versions fixes and user feedbacks, the production version is
expected for end of Q3/2022.

> Release before 1.0 should not be used for production purpose.
For the enhanced edition roadmap, please feel free to contact us at
<https://www.viridit.com/contact>.

## Release 0.13.x : available in july 2022

This version will focus on performances and message transfer optimisation.

- [ARC](https://datatracker.ietf.org/doc/html/rfc8617)/[BIMI](https://www.ietf.org/archive/id/draft-blank-ietf-bimi-02.txt) support.
- MySQL database support.
- An attachment caching system.
- Native integration of anti-viruses (CLAMAV)
- performance improvements.

## Release 0.12.x : available in june 2022

This version will focus on expending databases support and new security protocols.

- [DKIM](https://datatracker.ietf.org/doc/html/rfc6376)/[DMARC](https://datatracker.ietf.org/doc/html/rfc7489) support.
- Redis, Memcached & LDAP databases support
- [DANE](https://blog.apnic.net/2019/11/20/better-mail-security-with-dane-for-smtp/) support for vSMTP's transport system.

## Release 0.11.x : available in mai 2022

This version will focus on external services with The vSMTP Policy Server (vPS) and databases.

- implementation of The vSMTP Policy Server (vPS), a module dedicated to the integration of third-party software.
  - manages complex filtering and security rules for incoming emails.
  - accepts delegation trough the SMTP protocol via Unix & Inet sockets.

- databases support for VSL.
  - implementation of the databases access syntax.
  - support for file databases.

Additional features for this version:

- support of the [Null MX](https://datatracker.ietf.org/doc/html/rfc7505) record for vSMTP's transport.
- [SPF](https://datatracker.ietf.org/doc/html/rfc7208) support.
- DDOS, zombies and SPAM bots countermeasures with a shield module that bridges a client with the server.
- a vqueue program that will enable you to manipulate queues manually.
  - show the content of a queue.
  - move message from queues.
  - remove messages from queues.
  - try to send a message placed in quarantine or dead queues.

## Release 0.10.x : available in april 2022

This version will be the first pre-production release, focusing on performance, testing and the rule engine syntax.

- Configuration
  - Folders restrictions for user-defined quarantines.
  - configuration for virtual domains.
  - DNS configuration for vSMTP's transport system.
- Rule engine
  - a new server vsl api that will enable interaction with services and server configuration in rules.
  - stabilizing VSL's syntax.

- optimisation and performance improvements.

## Release 0.9.x : current version

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

## Release 0.8.x

- The delivery process and the related queues for local and remote mails.
- The Post-queue filtering.

About filtering functionalities :

- MIME compliancy (RFC 2045+) checks.
- Actions on MIME headers related to RFC 5322.
- Offline filtering stage (post-queue).
- Deliver stage related rules and actions.

## Release 0.7.x

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
