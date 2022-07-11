# Changelog

<!-- next-header -->

## [Unreleased] - ReleaseDate

### Changed

* delegation directives sets the X-VSMTP-DELEGATION
  header in the email to pick up where processing left of. (#438)
* refactored the queue system to make it simpler. (#438)
* multiple delegation directives can be used in a single
  stage with one or multiple services. (#438)
* delegation is available for `postq` & `delivery` only. (#438)

## [1.1.2] - 2022-07-07

### Added

* a `prepend_header` and `append_header` in `vsl` api to push front/back headers in message (#410).
* run the deliveries of message concurrently (by transport method maildir/mbox/...) (#425).

### Changed

* you can now add headers to a message at any stages (instead of preq an later) (#410).

### Fixed

* improve SPF policies (#400).
* produce an error at startup with invalid rules stages (#391).
* fixed a bug where the delivery system would place successfully sent emails into deferred queue when only one MX record was available (#427).

### Breaking Changes

* `vsl` api of SPF has changed (see the documentation <https://vsmtp.rs/start/configuration/hardening.html#using-the-spf-protocol>).
* split `check_relay` to `check_mail_relay` and `check_rcpt_relay` (#412).

## [1.1.0] - 2022-06-20
