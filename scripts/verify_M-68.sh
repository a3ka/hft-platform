#!/usr/bin/env bash
# Acceptance-гейт M-68 rev3 — MD-I-8: депт-серия пересчитывается на КАЖДОМ L2-событии.
#
# Предмет (замер, не память). `depth_series` пересчитывается ТОЛЬКО в ветке `MdPayload::L2Snapshot`
# (`crates/gateway/src/lib.rs:961-963`); ветка `L2Delta` (`:984-986`) двигает книгу и heatmap, а
# полосы — нет, дословно «депт-серия остаётся snapshot-only (M-22 семантика)». Снимки 1 Гц против
# дельт 100 мс ⇒ полоса отстаёт до секунды, дельта-хвост в серию не входит вовсе.
#
# ЧЕГО ЗДЕСЬ НЕТ И ПОЧЕМУ. Прежние редакции гейта чинили «дальность»: якобы биржа капит снапшот
# на `limit=5000` ≈ 1.3 %. Посылка ЛОЖНА (`A-018` §1.1): пейлоад `L2Snapshot` — бакетированная
# проекция НАШЕЙ diff-книги с обрезкой `MAX_REL_DIST = 0.60`, дальние уровни в нём ЕСТЬ.
# Состав записи не меняется, contract-RFC не требуется (`CT-I-2`).
#
# Круг 3 (`A-018` §2.3). Три пункта, названные арбитром отдельно, закрыты здесь:
#   • `cargo test --all` добавлен — паритет с CI целиком (`gates.md` §3), в rev2 его не было;
#   • шаг B ИСПОЛНЯЕТ мутацию `C-M68-1` в копии дерева, а не грепает строку;
#   • шаг D дёргает ТОТ рычаг: чекпоинт отвергается сверкой `GATEWAY_SCHEMA_VERSION`
#     (`:2901-2904`), а не `CKPT_SCHEMA_VERSION` (версия формата файла).
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

FAIL=0
step() { echo "=== $* ==="; }
chk() { if "$@"; then echo "PASS: $*"; else echo "FAIL: $*"; FAIL=$((FAIL + 1)); fi; }
chk_sh() { if bash -c "$1" >/dev/null 2>&1; then echo "PASS: $2"; else echo "FAIL: $2"; FAIL=$((FAIL + 1)); fi; }

BASE=$(git merge-base HEAD origin/main 2>/dev/null || echo "")
if [ -z "${BASE}" ]; then
  echo "FAIL: merge-base с origin/main не вычислен — шаги диапазона (H,I,J,K) судить не по чему"
  FAIL=$((FAIL + 1))
fi

# ── Паритет с CI (gates.md §3): гейт, который зеленее CI, — не гейт. Базовая тройка ЦЕЛИКОМ.
# Специализированные джобы в зону M-68 не бьют: ни `contracts`, ни процессный слой, ни номера
# артефактов предметом не трогаются.
step "task #0 — паритет с CI: fmt + clippy(--all-targets --all-features) + test --all"
chk cargo fmt --all -- --check
chk cargo clippy --all-targets --all-features -- -D warnings
chk cargo test --all --quiet

step "A (задачи 1,2,3,4,5,7) — набор MD-I-8 целиком"
chk cargo test -p gateway --test red_depth_from_book --quiet

# Состав НАЗВАН ЛИТЕРАЛОМ, а не `-ge «сколько нашлось»`: порог, отстающий от набора, есть
# ослабление наблюдения ОТСУТСТВИЯ — потеря оракула оставила бы шаг зелёным. Тот же класс,
# что `R-118` N-1 на M-65 и `TD-140` на M-61.
EXPECT_D=9
N_D=$(grep -cE '^fn md_i8_d[0-9]' crates/gateway/tests/red_depth_from_book.rs || echo 0)
if [ "${N_D}" -eq "${EXPECT_D}" ]; then
  echo "PASS: A состав набора — ${N_D} оракулов (ожидалось ровно ${EXPECT_D}: d1 d2 d3 d4 d5 d7 d7b d8 d8b)"
else
  echo "FAIL: A состав набора — ${N_D} при ожидаемых ${EXPECT_D}; порог и набор разошлись"
  FAIL=$((FAIL + 1))
fi

step "B (задача 4) — мутационный контроль ИСПОЛНЯЕТСЯ: набор обязан быть КРАСНЫМ против C-M68-1"
# Мутант критика (`C-094` B2): пересчитывать от книги только дальние полосы. Проверка
# ПОВЕДЕНИЕМ: мутация вносится в КОПИЮ дерева и НАБОР ПРОГОНЯЕТСЯ ТАМ (`branch-hygiene`:
# мутации — в отдельном дереве, оригинал не трогается). Шаг падает, если набор мутанта
# ПЕРЕЖИВАЕТ: значит он пиннит не то.
#
# Якорь мутации ЗАФИКСИРОВАН СПЕКОЙ (§4 задача 1, §3.1): реализация обязана нести приватный
# помощник `fn depth_from_book(&self, side: Side, band: f64) -> i64` с комментарием-маркером
# `MUT-ANCHOR C-M68-1` строкой выше. Без стабильного якоря мутацию нельзя ВНЕСТИ, а не только
# проверить — на этом rev2 и подменил прогон грепом.
#
# Что мутация воспроизводит, названо точно: «узкая полоса НЕ пересчитывается от книги».
# Значение, которое C-M68-1 оставлял узким полосам (snapshot-derived), она не воспроизводит —
# здесь они обнуляются. Для НАБОРА это эквивалентно: `d1`/`d4` сверяют ТОЧНОЕ значение каждой
# полосы, и любое непересчитанное её роняет. Разница названа, а не заглажена.
MUT=$(mktemp -d "${TMPDIR:-/tmp}/red-m68-mut-XXXXXX")
trap 'rm -rf "${MUT}"' EXIT
ANCHOR='MUT-ANCHOR C-M68-1'
if ! grep -q "${ANCHOR}" crates/gateway/src/lib.rs 2>/dev/null; then
  echo "FAIL: B SETUP НЕ СОСТОЯЛСЯ — якоря мутации '${ANCHOR}' в реализации НЕТ."
  echo "      Спека M-68 §4 задача 1 требует его: без якоря мутацию нельзя ВНЕСТИ, и шаг"
  echo "      выродился бы в греп (дефект rev2, C-138 п.2). До реализации это ожидаемо."
  FAIL=$((FAIL + 1))
elif ! cp -a crates Cargo.toml Cargo.lock "${MUT}/" 2>/dev/null; then
  echo "FAIL: B SETUP НЕ СОСТОЯЛСЯ — копия дерева не собралась"
  FAIL=$((FAIL + 1))
else
  # Вносим мутацию: узкие полосы перестают читаться из книги.
  perl -0pi -e 's/(MUT-ANCHOR C-M68-1.*?\n\s*fn depth_from_book\([^\n]*\{\n)/$1        if band < 0.60 { return 0; }\n/s' \
    "${MUT}/crates/gateway/src/lib.rs"
  if ! grep -q 'if band < 0.60 { return 0; }' "${MUT}/crates/gateway/src/lib.rs"; then
    echo "FAIL: B SETUP НЕ СОСТОЯЛСЯ — мутация в копию НЕ внесена (сигнатура помощника разошлась со спекой)"
    FAIL=$((FAIL + 1))
  elif (cd "${MUT}" && cargo test -p gateway --test red_depth_from_book --quiet >/dev/null 2>&1); then
    echo "FAIL: B набор ПЕРЕЖИЛ мутанта C-M68-1 — он не пиннит «каждая полоса», а лишь дальнюю"
    FAIL=$((FAIL + 1))
  else
    echo "PASS: B набор КРАСЕН против мутанта C-M68-1 (мутация внесена и прогнана в копии)"
  fi
fi

step "C (задача 8) — ресурсный оракул пути L2Delta → depth"
chk cargo test -p gateway --test red_depth_recompute_cost --quiet

step "D (задача 9) — смена СЕМАНТИКИ объявлена bump'ом GATEWAY_SCHEMA_VERSION"
# ИМЕННО этот рычаг: `read_and_validate` шаг (3) отвергает чекпоинт при
# `gw_v != GATEWAY_SCHEMA_VERSION` (`crates/gateway/src/lib.rs:2901-2904`).
# `CKPT_SCHEMA_VERSION` — версия ФОРМАТА ФАЙЛА, формат не меняется; rev2 требовал его и дёргал
# не тот рычаг. Bump закрывает разом `C-094` B3 (явная инвалидация) и `П-014` п.3.
chk_sh "test \"\$(grep -oE 'GATEWAY_SCHEMA_VERSION: u32 = [0-9]+' crates/gateway/src/lib.rs | grep -oE '[0-9]+\$')\" -ge 9" \
       "D GATEWAY_SCHEMA_VERSION >= 9 (на момент спеки было 8)"

step "E (задача 10) — VB-I-10 не ослаблен переходом на пересчёт по книге"
chk cargo test -p gateway --test red_gateway_bounded --quiet
chk cargo test -p gateway --test red_snapshot_noclone --quiet

step "F (задача 6) — VB-I-2 live == replay"
chk cargo test -p gateway --test red_gateway_live_eq_replay --quiet

step "G (задача 7) — метка и её числа сняты одним наблюдением; соседний инвариант не куплен"
# ДВЕ роли в одном шаге, и обе обязательны. (1) Два оракула этого файла rev3 ПЕРЕПИСАЛ —
# они часть RED-набора задачи 7 (спека §2bis). (2) Остальные семь — КОНТРОЛИ: они зелены
# сегодня и обязаны остаться зелёными; покрасневший контроль означает, что фикс куплен ценой
# соседнего инварианта (`testing.md` §«Второй вопрос: что пришлось ослабить рядом»).
chk cargo test -p gateway --test red_depth_provenance_by_reach --quiet

step "H — Block-C: contracts не тронуты предметом"
chk_sh "git diff --name-only ${BASE}..HEAD -- crates/contracts | grep -q . && exit 1 || exit 0" \
       "H crates/contracts не тронут"

step "I — состав ВЫДАЧИ не тронут: GATEWAY_BANDS остаётся прод-дефолтом"
# Включение полос — граница C и ОТДЕЛЬНЫЙ милестоун (`П-014` пп.1/3/4). M-68 чинит, КАК
# считается, а не ЧТО показывается.
chk_sh "git diff ${BASE}..HEAD -- docker-compose.yml | grep -q 'GATEWAY_BANDS' && exit 1 || exit 0" \
       "I GATEWAY_BANDS в docker-compose.yml не тронут"

step "J (C-094 B3) — selector_fingerprint не подогнан под кэш"
# Развязка обязана быть ЯВНОЙ инвалидацией (шаг D), а не подгонкой отпечатка: подгонка прячет
# смену смысла редьюсера и ломает VB-I-2 в warm-start пути.
chk_sh "git diff ${BASE}..HEAD -- crates/gateway/src/lib.rs | grep -E '^[+-].*fn selector_fingerprint' | grep -q . && exit 1 || exit 0" \
       "J selector_fingerprint не переписан"

step "K — зона предмета: чужие крейты и роадмап в диапазоне не участвуют"
# `crates/book` — примитивы `depth_within`/`max_reach_pct` достаточны (спека §3).
# `docs/09-roadmap-v2.md` — причина, по которой прежняя ветка терминальна (`C-094` B6):
# фазовое решение founder'а не едет контейнером в милестоуне.
chk_sh "git diff --name-only ${BASE}..HEAD -- crates/book crates/venue-binance crates/venue-binance-futures crates/journal docs/09-roadmap-v2.md | grep -q . && exit 1 || exit 0" \
       "K book/venue/journal/роадмап не тронуты диапазоном"

echo
if [ "${FAIL}" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (${FAIL})"; exit 1
