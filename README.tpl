[![build status](https://github.com/pwalski/tchannel_rs/actions/workflows/ci.yml/badge.svg)](https://github.com/pwalski/tchannel_rs/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](./LICENSE.md)

# {{crate}}

{{readme}}
## Build

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
# with local Maven/JDK
mvn -f examples-jvm-server package exec:exec -Pserver
# or with Docker
docker-compose --project-directory examples-jvm-server up
# or with Podman (no podman-compose because of network issues)
podman build --file examples-jvm-server/Dockerfile
podman run -p 8888:8888 localhost/examples-jvm-server_tchannel-jvm-server
```

### Update of README.md
```shell
cargo install cargo-readme
cargo readme > README.md
```

---

License: {{license}}
