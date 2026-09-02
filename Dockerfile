FROM rust:1-alpine AS builder
WORKDIR /app

RUN apk add --no-cache build-base musl-dev

COPY Cargo.toml Cargo.lock ./
COPY src-common/Cargo.toml src-common/Cargo.toml
COPY src-server/Cargo.toml src-server/Cargo.toml
COPY src-tauri/Cargo.toml src-tauri/Cargo.toml
COPY src-tauri/src/lib.rs src-tauri/src/lib.rs
COPY src-common/src src-common/src
COPY src-server/src src-server/src

RUN cargo build --locked --release --package friendolls-server

FROM alpine:3.23.5
RUN adduser -u 10001 -D friendolls
RUN mkdir /app

COPY --from=builder /app/target/release/friendolls-server /app/friendolls-server

USER friendolls
ENV BIND_ADDR=0.0.0.0:27520
EXPOSE 27520
ENTRYPOINT ["/app/friendolls-server"]
