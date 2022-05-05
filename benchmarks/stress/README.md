# Stress test of vSMTP

Usage :

```sh
# at the racine of the repository
$> cargo build --release --bin vsmtp
$> ./jaeger-all-in-one # (see https://www.jaegertracing.io/docs/1.33/getting-started/)
$> cargo run --bin vsmtp-stress
# or
$> CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph --deterministic --bin vsmtp-stress
```
