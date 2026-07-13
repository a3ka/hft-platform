#!/usr/bin/env bash
# verify_M-08 — acceptance-гейт milestone M-08 (Data durability + CT-RFC-02).
# Реальный гейт per .claude/rules/gates.md §3: FAIL-агрегатор + exit≠0, ≥1 проверка на задачу.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

STEP_LOG="$(mktemp -t verify_m08.XXXXXX.log)"
trap 'rm -f "${STEP_LOG}"' EXIT

FAILED=0
check() {
  local label="$1"; shift
  if "$@" >"${STEP_LOG}" 2>&1; then
    echo "PASS  ${label}"
  else
    echo "FAIL  ${label}"
    tail -20 "${STEP_LOG}"
    FAILED=$((FAILED + 1))
  fi
}

# T1: форма (все крейты — урок RN-8)
check "T1a cargo fmt --all --check" cargo fmt --all -- --check
check "T1b clippy --workspace -D warnings" cargo clippy --workspace --all-targets -- -D warnings

# T2 (задача 1): CT-RFC-02 — T1-формы, schema_version 2, legacy-вменение, стабильность дискриминантов
check "T2 contracts RED (CT-RFC-02)" cargo test -p contracts

# T3 (задача 2): ротация сегментов + сквозной seq + заголовок в каждом сегменте
check "T3 journal ротация (E2/CT-I-6)" cargo test -p journal --test red_rotation

# T4 (задача 2): ПРОД-МАСШТАБ — стрим bounded-memory + O(1) по размеру журнала.
# Отдельной строкой: именно этот оракул отделяет «работает на фикстурах» от «работает
# на боевых 8.3 GB» (урок TD-011 — юниты на килобайтах пропустили OOM).
check "T4 journal стрим bounded-memory (E5, прод-масштаб)" cargo test -p journal --test red_stream_bounded

# T5 (задача 2): legacy-сегмент читается вечно; эпохи не смешиваются молча
check "T5 journal legacy + эпохи (CT-RFC02-1..4)" cargo test -p journal --test red_segments_epochs

# T6 (задача 3): ретеншен (типовой барьер ColdCopyProof) + fail-closed по диску
check "T6 journal ретеншен + disk-guard (E3/E4)" cargo test -p journal --test red_retention

# T7 (задача 2/4): регрессия журнала — старые инварианты живы (DET-I-1, TD-011 bounded open)
check "T7 journal регрессия (вкл. red_open_bounded)" cargo test -p journal

# T8 (задача 4): recorder не сломан ротацией
check "T8 recorder тесты" cargo test -p recorder

# T9 (задача 5): грид на стриме — bounded memory + ЭКВИВАЛЕНТНОСТЬ in-memory прогону
check "T9 research-cli стрим-грид (E5, память + эквивалентность)" cargo test -p research-cli --test red_stream_grid
check "T9b research-cli регрессия (RC-I-*, GR-I-*)" cargo test -p research-cli

# T2b (C-005 C1): CT-I-4 — схема СГЕНЕРИРОВАНА из типов и согласована; фикстуры valid/invalid
check "T2b CT-I-4 схема+фикстуры (RFC-пакет)" cargo test -p contracts --test red_schema
check "T2c схема лежит в репо (05 §4)" bash -c 'ls crates/contracts/schema/*.schema.json >/dev/null'
check "T2d CHANGELOG контрактов ведётся" bash -c 'grep -q "CT-RFC-02" crates/contracts/CHANGELOG.md'

# T8b (E7): writer-цикл recorder'а bounded по памяти. ЗЕЛЁН на текущем коде — это РЕЗУЛЬТАТ:
# лик TD-016 не в writer'е. Оставлен как регресс-guard.
check "T8b recorder bounded-RSS (E7, regression guard)" cargo test -p recorder --test red_rss_bounded
# T8c (TD-016, КОРЕНЬ): книга venue-адаптера копит мёртвые уровни (100k→400k на 50k→200k
# диффов) → +6.5 MiB/час на проде. Границу ресурса гейтим тестом, а не глазами через 5 часов.
check "T8c venue-binance book bounded (TD-016 корень)" cargo test -p venue-binance --test red_book_bounded

# T9c (E8/C-005 M1): gap-статистика — тихие дыры и реконнект-дыры
check "T9c research-cli gap-статистика (E8)" cargo test -p research-cli --test red_gaps

# T10: регрессия верхних слоёв (M-07 мозг стратегии не тронут)
check "T10 регрессия alpha/portfolio/strategy/sim/signals/book" bash -c 'cargo test -p alpha -p portfolio -p strategy -p sim -p signals -p book'

# T11: структурные грепы
check "T11a имя сегмента НЕ захардкожено (только КОД, не комментарии)" bash -c 'grep -rn "segment-00000000" crates/journal/src/segments.rs crates/recorder/src 2>/dev/null | grep -vE "^[^:]+:[0-9]+: *(//|///|//!)" | grep -q . && exit 1 || exit 0'
check "T11b research-cli не читает журнал через read_all" bash -c '! grep -rn "journal::read_all\|read_all(&journal_dir)" crates/research-cli/src'
check "T11c research-cli называет эпоху явно (EpochFilter)" bash -c 'grep -rq "EpochFilter" crates/research-cli/src'
check "T11d T1-формы CT-RFC-02 определены ровно в contracts" bash -c '[ "$(grep -rln "pub struct SegmentHeader" crates/*/src | wc -l)" -eq 1 ]'

# T12 (задача 6): деплой гейтится на CI — красный CI не выкатывает прод
check "T12 deploy: jobs.deploy.needs содержит ci (структурно)" python3 scripts/check_deploy_gate.py

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} провалов)"
  exit 1
fi
echo "VERDICT: PASS"
