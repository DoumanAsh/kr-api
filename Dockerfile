FROM ekidd/rust-musl-builder:latest as build

ADD --chown=rust:rust . .

RUN cargo build --release

# Minimize overhead to execute

FROM alpine:latest

COPY --from=build home/rust/src/target/x86_64-unknown-linux-musl/release/kr-api kr-api

RUN ls /usr/local/bin/

CMD ./kr-api
