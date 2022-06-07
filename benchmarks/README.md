# Stress test for vSMTP

This folder contains a binary `vsmtp-stress` simulating heavy message traffic.

The program will instantiate `X` client on separated threads, each sending `Y` mails.\
If a client failed (code 5xx) to send emails, it will try to send a full payload later.

This program helps to monitor 3 characteristics :

* **Utilization** is the amount of time the system is actually doing useful work servicing a request.
* **Saturation** is when requests have to wait before being serviced.
* **Errors** are when things start to fail, when queues are no longer able to accept any new requests for example.

## Usage

> All commands are run from the root of the repository.
> Do not forget to empty the queues !

### Generate a flamegraph

```sh
cargo build --bin vsmtp-stress --manifest-path=benchmarks/stress/Cargo.toml
CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph --deterministic \
    --bin vsmtp -- -t 10s --no-daemon -c benchmarks/stress/vsmtp.stress.toml &
./benchmarks/stress/target/debug/vsmtp-stress
```

### Generate telemetries

```sh
cargo build --release --bin vsmtp
cargo build --bin vsmtp-stress --features telemetry --manifest-path=benchmarks/stress/Cargo.toml
jaeger-all-in-one & # (see <https://www.jaegertracing.io/docs/1.33/getting-started/>)
cargo run --release --bin vsmtp -- -t 10s --no-daemon -c benchmarks/stress/vsmtp.stress.toml &
./benchmarks/stress/target/debug/vsmtp-stress
```

### Measure CPU instructions

```sh
cargo build --bin vsmtp-stress --manifest-path=benchmarks/stress/Cargo.toml
./tools/instructions.sh
./benchmarks/stress/target/debug/vsmtp-stress
```

## Benchmarks

`smtp-source` is used to mesure vsmtp's performances.

```sh
# smtp-source is included with postfix.
sudo apt install postfix
```

You can use the following command to simulate incoming clients.

```sh
time smtp-source -s <nbr-of-sessions>    \
                 -l <message-size>       \
                 -m <nbr-of-messages>    \
                 -f <sender-address>     \
                 -N                      \
                 -t <recipient-address>  \
                 127.0.0.1:25
```

To reproduce the README's benchmarks, you can use the following configuration for vsmtp:

```toml
# copy-paste this to `/etc/vsmtp/vsmtp.toml`
version_requirement = ">=1.0.0"

[server]
client_count_max = -1

[server.logs.level]
default = "error"

[server.interfaces]
addr = ["127.0.0.1:25"]
addr_submission = ["127.0.0.1:587"]
addr_submissions = ["127.0.0.1:465"]

[server.system.thread_pool]
receiver = 6
processing = 6
delivery = 6

[app.vsl]
filepath = "/etc/vsmtp/rules/main.vsl"
```

```rust
// copy-paste this to `/etc/vsmtp/rules/main.vsl`
#{
    preq: [ rule "hold emails" || quarantine("bench") ]
}
```

And the following configuration for postfix:

```
# copy-paste this to `/etc/postfix/main.cf`
smtpd_banner = $myhostname ESMTP $mail_name

compatibility_level = 2

smtpd_client_restrictions = permit_mynetworks
smtpd_recipient_restrictions = static:hold

message_size_limit = 200000000

myhostname = host.example.com
mydestination = postfix.com
mynetworks = 127.0.0.0/24
```

In the readme benchmarks, we used `systemctl` to run postfix & vsmtp as services.
```sh
# using systemctl

sudo systemctl start postfix.service
## or
sudo systemctl start vsmtp.service
```

Do not forget to empty you queues between each run of `smtp-source`, as they might clog up your filesystem really fast.

```sh
# Empty vsmtp's spool.
rm /var/spool/vsmtp/mails/*
rm /var/spool/vsmtp/app/benches*
# Empty postfix's hold queue.
rm /var/spool/postfix/hold/*
```
