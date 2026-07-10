# M-00: сборка recorder-заглушки. Multi-stage: build (rust) -> slim runtime.
# Реальный recorder (HL WS -> журнал) появится в M-01; Dockerfile расширится (доп. крейты).
FROM rust:1-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --bin recorder

FROM debian:stable-slim
# journal-том монтируется сюда; переживает редеплой контейнера (docs/06 §3, §7).
ENV JOURNAL_DIR=/journal
RUN mkdir -p /journal
VOLUME ["/journal"]
COPY --from=builder /build/target/release/recorder /usr/local/bin/recorder
# M-00: работаем root'ом (заглушка). Hardening (non-root + права тома) — TODO при реальном recorder.
ENTRYPOINT ["/usr/local/bin/recorder"]
