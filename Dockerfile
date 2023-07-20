FROM rust:latest as builder

WORKDIR /app

COPY . .

ENV SQLX_OFFLINE=true

RUN cargo build --release

FROM debian:bookworm-slim

COPY --from=builder /app/target/release/claymore_backend /usr/local/bin/claymore_backend

CMD ["claymore_backend"]
