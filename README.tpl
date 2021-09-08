[![build status](https://github.com/pwalski/tchannel-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/pwalski/tchannel-rust/actions)

# {{crate}}

{{readme}}

## Build

Update of README
```shell
cargo install cargo-readme
cargo readme > README.md
```

## Examples Subproject

Sample `tchannel-java` server:
```shell
mvn -f examples-server package exec:exec -Pserver
```

Basic client scenario:
```shell
RUST_LOG=DEBUG cargo run --example client
```

---

License: {{license}}
