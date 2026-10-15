# syntax=docker/dockerfile:1
FROM python:3.13-slim-bookworm AS java
WORKDIR /source
COPY scripts/prepare_java_runtime.py ./scripts/prepare_java_runtime.py
COPY vendor/java-runtime/manifest.json ./vendor/java-runtime/manifest.json
RUN python scripts/prepare_java_runtime.py

FROM node:22-bookworm-slim AS web
WORKDIR /source/web
COPY web/package.json web/package-lock.json ./
COPY web/patches/ ./patches/
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1-bookworm AS rust
RUN apt-get update && apt-get install -y --no-install-recommends python3 ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /source
COPY Cargo.toml Cargo.lock ./
COPY vendor/ ./vendor/
COPY crates/ ./crates/
COPY scripts/prepare_openapi_generator.py ./scripts/prepare_openapi_generator.py
RUN python3 scripts/prepare_openapi_generator.py
COPY desktop/ ./desktop/
COPY --from=web /source/web/dist ./web/dist/
RUN cargo build --locked --release -p moleapi-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libstdc++6 zlib1g && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 moleapi && useradd --uid 10001 --gid 10001 --no-create-home moleapi \
    && mkdir /data && chown moleapi:moleapi /data
COPY --from=rust /source/target/release/moleapi-server /usr/local/bin/moleapi-server
COPY --from=java /source/vendor/java-runtime/current /opt/moleapi/runtime/java
USER 10001:10001
ENV MOLEAPI_BIND=0.0.0.0:8787
ENV MOLEAPI_CODEGEN_JAVA=/opt/moleapi/runtime/java/payload/bin/java
WORKDIR /data
VOLUME ["/data"]
EXPOSE 8787
ENTRYPOINT ["moleapi-server"]
CMD ["--database", "/data/moleapi.db"]
