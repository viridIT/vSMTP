# Stress test for vSMTP

This folder contains a bin `vsmtp-stress` simulating a heavy traffic.

The program will instantiate `X` client on separated thread, each sending `Y` mails.\
If a client failed (code 5xx) to send a mails, he will try to send a full payload latter.

This program helps to monitor 3 characteristics :

* **Utilization** is the amount of time the system is actually doing useful work servicing a request
* **Saturation** is when requests have to wait before being serviced
* **Errors** are when things start to fail, like when queues are no longer able to accept any new requests

## Usage

> All the command run from the racine of the repository
> And don't forget to empty the queues !!

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
