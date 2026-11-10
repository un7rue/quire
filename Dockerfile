# Quire's web server: the same core and UI as the desktop app, without Tauri.
#
#   docker build -t quire-server .
#   docker run -d -p 8766:8766 -v quire-data:/data -e QUIRE_SECRET_KEY='<a long passphrase>' quire-server
#
# Put it behind an HTTPS proxy and add `--secure` (the session cookie then only travels over HTTPS).
# Data sources read files from /data (uploads) and from folders given with `--allow-dir` (mount them);
# `--allow-host db.bank.local` limits the database servers it connects to. The report API is at
# /api/v1/ (Admin → API makes the keys); `--api-workers N` reports are made at once (2).

FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY resources ./resources
COPY ui/fonts ./ui/fonts
# --features render: the report API draws pages in V8 (prebuilt, downloaded by the v8 crate)
RUN cargo build --release --no-default-features --features render --bin quire-server

FROM debian:bookworm-slim
# curl: the AI providers and ClickHouse are reached through it; python3 + pandas: Python data sources
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl python3 python3-pandas \
 && apt-get clean \
 && useradd --system --create-home --home-dir /home/quire quire \
 && mkdir /data && chown quire /data
COPY --from=build /src/target/release/quire-server /usr/local/bin/quire-server
COPY ui /app/ui
COPY data/demo.duckdb /app/demo/demo.duckdb
ENV QUIRE_UI=/app/ui QUIRE_DEMO_DB=/app/demo/demo.duckdb QUIRE_DATA=/data QUIRE_PYTHON=/usr/bin/python3
USER quire
VOLUME /data
EXPOSE 8766
ENTRYPOINT ["quire-server", "--host", "0.0.0.0", "--port", "8766"]
