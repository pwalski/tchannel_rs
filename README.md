# tchannel-rust

## Run

Sample `tchannel-java` server:
```shell
mvn -f tchannel-example/server package exec:exec -Pserver
```

Basic client scenario:
```shell
RUST_LOG=DEBUG cargo run -p tchannel-example --example basic
```
