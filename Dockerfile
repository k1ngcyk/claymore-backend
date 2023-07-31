FROM rust:latest as builder

WORKDIR /app

COPY . .

ENV SQLX_OFFLINE=true

RUN cargo install sqlx-cli --features postgres

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates

RUN update-ca-certificates

RUN mkdir -p /opt/fluxus

COPY migrations /opt/fluxus/migrations

COPY --from=builder /app/target/release/claymore_backend /opt/fluxus/claymore_backend

CMD ["/opt/fluxus/claymore_backend"]
