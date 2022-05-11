# vSMTP standard edition roadmap

## Production release

Depending on versions fixes and user feedbacks, the production version is expected for end of Q3/2022.

> Release before should not be used for production purpose.

## Current version : 1.0.0

 This version is the first pre-production release.

- Databases support for VSL.
  - implementation of the databases access syntax.
  - support for file databases.
- Queues and quarantines management.

## Planned features and releases

### Release 1.1.x : May 2022

- Security delegation via SMTP.
- support of the [Null MX](https://datatracker.ietf.org/doc/html/rfc7505) record.
- [SPF](https://datatracker.ietf.org/doc/html/rfc7208) support.

### Release 1.2.x : June 2022

- [DKIM](https://datatracker.ietf.org/doc/html/rfc6376) and [DMARC](https://datatracker.ietf.org/doc/html/rfc7489) support.
- [DANE](https://blog.apnic.net/2019/11/20/better-mail-security-with-dane-for-smtp/) support for vSMTP's transport system.
- SQL database support.
- Performance improvement : connection caches

### Release 1.3.x

- Redis, Memcached & LDAP databases support.
- Security shield : DDoS, zombies and SPAM bots countermeasures.
- Performance improvement : content caches

## Unplanned features

- Direct connections to anti-virus (ClamAV, Sophos, etc.) through internal plugins
- [ARC](https://datatracker.ietf.org/doc/html/rfc8617) support.
- [BIMI](https://www.ietf.org/archive/id/draft-blank-ietf-bimi-02.txt) support.

## Older releases

### Releases 0.10.x

- Configuration improvements.
  - Folders restrictions for user-defined quarantines.
  - configuration for virtual domains.
  - DNS configuration for vSMTP's transport system.
- Rule engine new features.
  - a new server vsl api that will enable interaction with services and server configuration in rules.
  - stabilizing VSL's syntax.
- optimisation and performance improvements.

### Releases 0.9.x

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

### Releases 0.8.x

- The delivery process and the related queues for local and remote mails.
- The Post-queue filtering.

About filtering functionalities :

- MIME compliancy (RFC 2045+) checks.
- Actions on MIME headers related to RFC 5322.
- Offline filtering stage (post-queue).
- Deliver stage related rules and actions.

### Release 0.7.x

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
