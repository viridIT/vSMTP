<h1 align="center">vSMTP 📫</h1>
<div align="center">
  <strong>
    The next-gen MTA. Secured, Faster and Greener
  </strong>
</div>

<br />

<div align="center">
  <a href="https://www.viridit.com">
    <img src="https://img.shields.io/badge/visit-website-green"
      alt="website" />
  </a>
    <a href="https://www.gnu.org/licenses/gpl-3.0">
    <img src="https://img.shields.io/badge/License-GPLv3-blue.svg"
      alt="License GPLv3" />
  </a>
</div>

<div align="center">
  <a href="https://github.com/viridIT/vSMTP/actions/workflows/ci.yaml">
    <img src="https://github.com/viridIT/vSMTP/actions/workflows/ci.yaml/badge.svg?branch=rc-01"
      alt="CI" />
  </a>
</div>

---

vSMTP is a next-gen Mail Transfer Agent (MTA) developed by viridIT teams.
You can follow us on <https://www.viridit.com>

## What is vSMTP ?

Whereas optimizing allocated resources is becoming a growing challenge, computer attacks remain a constant issue.
Over 300 billion emails are sent and received in the world every day. Billions of attachments are processed, analyzed and delivered, contributing to the increase in greenhouse gas emissions.
To meet this challenge, viridIT is developing a new technology of email gateways, also called vSMTP.

## Why vSMTP is your future SMTP server ?

- Developed in Rust, implying high performance and stability
- Modular and highly customizable
- It has a a complete filtering system
- Actively maintained and developed

Because it is secured, faster and greener.

## Documentation

About the code and related issues, please check the [project Wiki](https://github.com/viridIT/vSMTP/wiki) and use the GitHub issue tracker.
To stay tuned, ask questions and get in-depth answers feel free to register and visit our forums at <https://www.viridit.com/community-forum>.
For documentation, user guide, etc. please consult GitHub wiki or our dedicated page at <https://www.viridit.com/support>
For any question related to commercial, licensing, etc. you can join us at <https://www.viridit.com/contact>

## Roadmap

vSMTP is currently under development. The current version "RC1" focus on the SMTP connection and state machine. You can find more information about the project agenda in the [ROADMAP](https://github.com/viridIT/vSMTP/blob/main/ROADMAP.md).

## Testing Policies

Linting

```sh
cargo lint
```

Unit test / Integration test

```sh
cargo +nightly test
```

Benchmarking

```sh
cargo bench
```

Fuzzing

```sh
cargo +nightly fuzz run
```

## License

The standard version of vSMTP is free and under an Open Source license.

It is provided as usual without any warranty.
Please refer to the LICENSE file for further informations.
