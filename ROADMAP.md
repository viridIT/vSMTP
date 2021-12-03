# vSMTP standard edition roadmap

>Release candidate versions should not be use for production purpose.  

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

- Deploy the delivery process (local/remote, delivery queues, etc.)
- Allow standard/enhanced edition switching
- Post-queue filtering
- Syslog

About filtering :

- MIME compliancy (RFC 2045+) checks
- Actions on MIME headers related to RFC 5322
- Offline filtering stage (post-queue)
- Deliver stage related rules and actions

## Release Candidate 3

This release will focus on :

- Performances
- The vSMTP policy server

About vSMTP policy server:  

vSMTP Policy Server (vPS) is a module dedicated to integrating of third-party software.  
Thanks to its logic engine it can handle complex filtering and security rules over several network

il offre à l'utilisateur la possibilité de...

It interacts through a logical engine which main functionalities are:
- the possibility of handling complex chains of software
- A libmilter-like emulator
- A Postfix-like policy server through SMTP protocol
- A Protobuf/GRPC 
- HTTP
- Local Unix scripts


It can be called at any stage of a SMTP transaction.  

## Version 1.0

Available on : Q2/2022

- 3rd party library for remote delivery

- ClamAV basic integration

### Configuration
- Network configuration parameters  
- Security and DDoS parameters
