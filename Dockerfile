FROM node:22-bookworm-slim AS ui-builder

WORKDIR /app/strawtea-ui

COPY strawtea-ui/package.json strawtea-ui/package-lock.json ./
RUN npm ci

COPY strawtea-ui/ ./
RUN npm run check && npm run build

FROM rust:1.88-bookworm AS backend-builder

WORKDIR /app/strawtea-be

COPY strawtea-be/Cargo.toml strawtea-be/Cargo.lock ./
COPY strawtea-be/migrations ./migrations
COPY strawtea-be/src ./src

RUN cargo build --locked --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --home-dir /app --shell /usr/sbin/nologin strawtea

WORKDIR /app

COPY --from=backend-builder /app/strawtea-be/target/release/strawtea-be /app/strawtea-be
COPY --from=ui-builder /app/strawtea-ui/dist /app/public
COPY deploy/railway-entrypoint.sh /app/railway-entrypoint.sh

RUN chmod 0755 /app/railway-entrypoint.sh \
    && chown -R strawtea:strawtea /app

USER strawtea

ENV STATIC_DIR=/app/public

EXPOSE 8080

CMD ["/app/railway-entrypoint.sh"]
