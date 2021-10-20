[![build status](https://github.com/pwalski/tchannel-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/pwalski/tchannel-rs/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](./LICENSE.md)

# {{crate}}

{{readme}}
## Build

### Update of README.md
```shell
cargo install cargo-readme
cargo readme > README.md
```

### Examples Subproject

Sample server:
```shell
RUST_LOG=DEBUG cargo run --example server
```

Sample client:
```shell
RUST_LOG=DEBUG cargo run --example client
```

Sample `tchannel-java` server (to check Rust client compatibility):
```shell
mvn -f examples-jvm-server package exec:exec -Pserver
# or with Docker
docker-compose --project-directory examples-jvm-server up
```

---

License: {{license}}
