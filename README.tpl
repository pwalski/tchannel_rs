![ci](https://github.com/pwalski/tchannel-rust/actions/workflows/ci.yml/badge.svg)

# {{crate}}

{{readme}}

## Build

Update of README
```shell
cargo install cargo-readme
cargo readme -r tchannel -t ../README.tpl > README.md
```

## Examples Subproject

Sample `tchannel-java` server:
```shell
mvn -f tchannel-examples/server package exec:exec -Pserver
```

Basic client scenario:
```shell
RUST_LOG=DEBUG cargo run -p tchannel-examples --example basic
```

---

License: {{license}}
