FROM rust:1.75-alpine as builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./

RUN cargo fetch

COPY src ./src
COPY config.toml .

RUN cargo build --release

FROM alpine:3.19

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/release/corrosion-monitor-backend /usr/local/bin/
COPY config.toml /etc/corrosion-monitor/config.toml

CMD ["corrosion-monitor-backend"]

EXPOSE 8080
