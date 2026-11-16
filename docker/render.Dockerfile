# Local only (never published without the legal check in PLAN-R1 §7).
# Build it on a build machine, not on a shared workstation: docker build -f docker/render.Dockerfile .
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl xz-utils tar && rm -rf /var/lib/apt/lists/*
COPY target/x86_64-unknown-linux-musl/release/scriptvox /usr/local/bin/scriptvox
COPY licenses/THIRD_PARTY.md LICENSE LICENSES/OFL-1.1.txt /usr/share/doc/scriptvox/third_party/
# FFmpeg and uv are fetched at first use from toolchain.lock; see SOURCES.md for the GPL sources.
ENV SCRIPTVOX_DATA=/var/lib/scriptvox
RUN scriptvox setup --json
ENTRYPOINT ["scriptvox"]
