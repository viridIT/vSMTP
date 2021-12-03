# vSMTP standard edition roadmap

>Release candidate versions should not be used for production purpose.  

More details about installation, scripting language and supported features can be found in the project wiki pages. https://github.com/viridIT/vSMTP/wiki
Register to your community, ask questions and get in-depth answers at https://www.viridit.com/community-forum/

For the enhanced edition roadmap, please feel free to contact us at <https://www.viridit.com/contact/>.

## Release Candidate 1

Available on December 2021, 6th. this release focuses on:

- Handling network connections, including TLS support
- SMTP compliancy (RFC 5321/5322)
- Interaction with the SMTP transaction
- Scripting language (vSL) and configuration files
- Local delivery using "maildir" (IMAP) protocol
- Application logs

About filtering :

- All SMTP state : HELO/EHLO, CONNECT, MAIL, RCPT, DATA
- Inline filtering (pre-queue)
- Basic actions (accept, deny, rewrite, etc.)
- User defined quarantine queues and logs
- JSON and RAW exports

## Release Candidate 2

Available in January 2022, version RC2 main objectives are:

- The delivery process (local/remote, delivery queues, etc.)
- Permit standard/enhanced edition switching
- Post-queue filtering
- Syslog

About filtering:

- MIME compliancy (RFC 2045+) checks
- Actions on MIME headers related to RFC 5322
- Offline filtering stage (post-queue)
- Deliver stage related rules and actions

## Release Candidate 3

This release will focus on:

- The vSMTP policy server
- Performances

About vSMTP policy server:

The vSMTP Policy Server (vPS) is a module dedicated to integrating of third-party software. Thanks to its logic engine it can manage complex filtering and security rules. In version RC3, vPS will accept delegation trough :

- The SMTP protocol
- A libmilter-like emulator
- Local Unix scripts

It can be called at any stage of a SMTP transaction via the rule engine.

## Production release

Depending on Release Candidate versions fixes and user feedbacks, the production version is expected for Q2/2022.
