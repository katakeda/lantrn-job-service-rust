FROM rust:1.67.1-slim as builder
WORKDIR /app
RUN apt update && apt install lld clang pkg-config libssl-dev -y
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim as runtime
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends cron openssl ca-certificates \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/lantrn-job-service /app/lantrn-job-service
COPY docker-entrypoint.sh .
RUN chmod +x /app/docker-entrypoint.sh
RUN crontab -l | { cat; echo "* * * * * /app/lantrn-job-service > /proc/1/fd/1 2>/proc/1/fd/2"; } | crontab -
ENTRYPOINT ["/app/docker-entrypoint.sh"]