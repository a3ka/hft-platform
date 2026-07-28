# M-00 + M-08 rev 7 (TD-020 доставка): multi-stage — build (rust) → slim runtime.
#
# ОДИН образ для recorder И journal-retention (D1: формат журнала один на проде).
# Расхождение версий кода между образами = порча/потеря данных (recorder пишет, ретеншен
# читает+удаляет — тот же wire-format, тот же v2-заголовок, тот же CRC32).
#
# ENTRYPOINT = recorder (D2: сбор важнее уборки; ops-сервис ретеншена поднимается отдельно,
# не может уронить сбор). `journal-retention` запускается через `docker compose run --rm`
# с override entrypoint.
FROM rust:1-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# D1: ОБА бинаря собираются и копируются в runtime-образ.
RUN cargo build --release --bin recorder --bin journal-retention --bin gateway-serve --bin gateway-checkpoint

FROM debian:stable-slim
# journal-том монтируется сюда; переживает редеплой контейнера (docs/06 §3, §7).
ENV JOURNAL_DIR=/journal
# D3: точка монтирования холодного хранилища (CIFS, fstab на ХОСТЕ → bind-mount в контейнер).
# Если холодное хранилище недоступно — verify_cold_copy падает, prune запрещён (ColdCopyProof),
# exit 2 (fail-closed: данные не удаляются «в никуда»).
ENV JOURNAL_COLD_DIR=/cold
RUN mkdir -p /journal /cold
VOLUME ["/journal"]
COPY --from=builder /build/target/release/recorder /usr/local/bin/recorder
COPY --from=builder /build/target/release/journal-retention /usr/local/bin/journal-retention
COPY --from=builder /build/target/release/gateway-serve /usr/local/bin/gateway-serve
COPY --from=builder /build/target/release/gateway-checkpoint /usr/local/bin/gateway-checkpoint
# M-00: работаем root'ом (заглушка). Hardening (non-root + права тома) — TODO при реальном recorder.
ENTRYPOINT ["/usr/local/bin/recorder"]
