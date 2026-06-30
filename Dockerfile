# syntax=docker/dockerfile:1.7

############################
# Stage 1: frontend build  #
############################
FROM node:22-bookworm-slim AS web
WORKDIR /web
# Cache deps: copy manifest + lockfile first. package-lock.json is committed -> npm ci is valid.
COPY web/package.json web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci
# tsc/vite/tailwind are devDependencies -> DO NOT use --omit=dev.
COPY web/ ./
RUN npm run build   # tsc --noEmit && vite build -> /web/dist

#########################################
# Stage 2: Rust build (native to host)  #
#########################################
# edition 2024 + rust-version 1.94 -> needs Rust >= 1.94. Debian/glibc to match runtime.
FROM rust:1.94-slim-bookworm AS build
WORKDIR /app
# Warm crate cache from the lockfile before copying source.
COPY Cargo.toml Cargo.lock ./
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo fetch --locked
# Build inputs that must exist BEFORE cargo build:
#   src/        (the crate)
#   migrations/ (embedded by sqlx::migrate!("./migrations"))
#   web/dist/   (embedded by rust-embed #[folder = "web/dist"])  <- from stage 1
COPY src ./src
COPY migrations ./migrations
COPY --from=web /web/dist ./web/dist
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --frozen --bin unpopularr
# release profile already sets strip=true.

#########################################
# Stage 3: minimal runtime              #
#########################################
FROM debian:bookworm-slim AS runtime
# ca-certificates: outbound HTTPS (reqwest/rustls) to *arr/Tautulli instances.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN groupadd --gid 10001 unpopularr \
    && useradd --uid 10001 --gid 10001 --no-create-home --shell /usr/sbin/nologin unpopularr
WORKDIR /app
# The binary embeds frontend + migrations, so it is the only artifact needed.
COPY --from=build /app/target/release/unpopularr /usr/local/bin/unpopularr
RUN mkdir -p /app/data && chown -R unpopularr:unpopularr /app
USER unpopularr
VOLUME ["/app/data"]
EXPOSE 3000
# CONFIG: unpopularr reads ./config.toml (cwd /app) or $UNPOPULARR_CONFIG. No config is baked in.
# The mounted config.toml MUST set `bind = "0.0.0.0:3000"` (the bind addr is config-file only,
# with no env override) or the port is unreachable from outside the container.
ENTRYPOINT ["/usr/local/bin/unpopularr"]
