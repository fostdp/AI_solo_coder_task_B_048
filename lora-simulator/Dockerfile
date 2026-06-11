FROM rust:1.75-alpine as builder

WORKDIR /app

COPY Cargo.toml ./

RUN cargo generate-lockfile

COPY src ./src

RUN cargo build --release

FROM alpine:3.19

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/release/lora-simulator /usr/local/bin/

CMD ["lora-simulator"]
