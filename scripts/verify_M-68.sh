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
# помощник `fn depth_from_book(&self, levels: &[(i64, i64)], mid: i64, bands: &[f64])
#   -> (Vec<i64>, u64)` с комментарием-маркером `MUT-ANCHOR C-M68-1` строкой выше.
# Сигнатура принимает ВСЕ полосы разом — это требование задачи 8 (`d6b`: цена не множится на
# число полос), а не стилистика: по-полосный помощник делал бы однопроходную реализацию
# невыразимой (блокер `C-156` F1).
# Без стабильного якоря мутацию нельзя ВНЕСТИ, а не только проверить — на этом rev2 подменил
# прогон грепом.
#
# Мутация обнуляет ширину узких полос ⇒ их суммы становятся нулевыми, дальняя считается как
# прежде. Это ровно класс `C-M68-1` («обновляем подмножество полос»). Значение, которое
# `C-M68-1` оставлял узким полосам (snapshot-derived), не воспроизводится — здесь они нули;
# для НАБОРА это эквивалентно: `d1`/`d4` сверяют ТОЧНОЕ значение каждой полосы, и любое
# непересчитанное её роняет. Разница названа, а не заглажена.
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
  perl -0pi -e 's/(MUT-ANCHOR C-M68-1.*?\n\s*fn depth_from_book\([^\n]*\{\n)/$1        let bands: Vec<f64> = bands.iter().map(|b| if *b < 0.60 { 0.0 } else { *b }).collect(); let bands = &bands[..];\n/s' \
    "${MUT}/crates/gateway/src/lib.rs"
  if ! grep -q 'let bands: Vec<f64> = bands.iter().map' "${MUT}/crates/gateway/src/lib.rs"; then
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

step "C2 (задачи 13,14 — R-134 B-3/B-4) — вырожденный вход и честность счётчика"
chk cargo test -p gateway --test red_depth_semantics --quiet
EXPECT_S=3
N_S=$(grep -cE "^fn md_i8_d(9|10)" crates/gateway/tests/red_depth_semantics.rs || true); N_S=${N_S:-0}
if [ "${N_S}" -eq "${EXPECT_S}" ]; then
  echo "PASS: C2 состав набора — ${N_S} оракулов (ожидалось ровно ${EXPECT_S}: d9 d9-C d10)"
else
  echo "FAIL: C2 состав набора — ${N_S} при ожидаемых ${EXPECT_S}; порог и набор разошлись"
  FAIL=$((FAIL + 1))
fi

step "C3ter (задача 23 — R-141 Б-1) — ИНВЕНТАРЬ: прод-писатель чекпоинта не вшивает каденцию"
# ЧТО ЭТО ПИННИТ. Имя файла чекпоинта есть отпечаток селектора, а задача 18 внесла в отпечаток
# depth_cadence_ms. Если писатель (gateway-checkpoint) вшивает `None`, а читатель
# (gateway-serve) после задачи 22 просит `Some(1000)`, имена расходятся, слепок не находится и
# журнал читается ЦЕЛИКОМ при каждом подключении (класс TD-044/R-029; замер R-141: 120 из 120
# против 0 при совпадении).
#
# ПОЧЕМУ ИНВЕНТАРЬ, А НЕ ОРАКУЛ — предел назван честно. Композиция «бинарь ↔ служба» наблюдаема
# только запуском ПРОЦЕССА с argv из docker-compose (testing.md, канарейка точки входа), а из
# интеграционного теста Rust она недостижима: selector_fingerprint и ckpt_path_for — pub(super).
# Первая редакция этого шага была ОРАКУЛОМ d19 и оказалась ТАВТОЛОГИЕЙ: он строил селектор
# писателя сам, в тесте, и остался бы красным при любом исправлении бинаря. Поймано пробой
# достижимости; ложный оракул удалён, а не отгружен. Настоящая канарейка процесса — отдельная
# работа, названная в §0sexies.2sexies.
#
# Барьер ЛЕКСИЧЕСКИЙ и COGNITIVE-ONLY в части доставки: ловит вшитый литерал, не доказывает,
# что значения совпадут в рантайме.
if grep -qE 'depth_cadence_ms:\s*None' crates/gateway/src/bin/gateway-checkpoint.rs; then
  echo "FAIL: C3ter прод-писатель чекпоинта ВШИВАЕТ depth_cadence_ms: None"
  echo "      (crates/gateway/src/bin/gateway-checkpoint.rs). Читатель после задачи 22 просит"
  echo "      Some(1000) — отпечатки расходятся, слепок не находится, журнал читается целиком"
  echo "      при каждом подключении. Задача 23: писатель получает ту же ручку."
  FAIL=$((FAIL + 1))
else
  echo "PASS: C3ter прод-писатель чекпоинта не вшивает каденцию (инвентарь, не оракул — см. комментарий)"
fi

step "C3bis (задача 22 — R-138 Б-3) — ручка каденции есть КОНФИГ: доходит, отказывает на мусоре, объявлена в compose"
chk cargo test -p gateway-serve --test red_depth_cadence_from_env --quiet

# Состав пиннится числом: оракул точки входа состоит из ЧЕТЫРЁХ проверок, и потеря любой
# делает остальные вакуумными. Без композиции доставки (`knob_is_declared_in_compose`) ручка
# зелена в юнитах и недоступна оператору — ровно класс, ради которого задача заведена.
EXPECT_E=6
N_E=$(grep -cE "^fn [a-z0-9_]+\(\) \{" crates/gateway-serve/tests/red_depth_cadence_from_env.rs || true); N_E=${N_E:-0}
if [ "${N_E}" -eq "${EXPECT_E}" ]; then
  echo "PASS: C3bis состав набора — ${N_E} оракулов (ожидалось ровно ${EXPECT_E}: доходит, дефолт≡пусто≡отсутствие, отказ на мусоре, объявлена у ОБЕИХ служб, d18e невыравненная пара, d18f carve-out)"
else
  echo "FAIL: C3bis состав набора — ${N_E} при ожидаемых ${EXPECT_E}; порог и набор разошлись"
  FAIL=$((FAIL + 1))
fi

step "C3 (задачи 15,16 + C-167) — каденция управляет, объявлена, представима, инвалидирует чекпоинт"
chk cargo test -p gateway --test red_depth_cadence --quiet
EXPECT_C=7
N_C=$(grep -cE "^fn md_i8_d(1[2-9]|20)_" crates/gateway/tests/red_depth_cadence.rs || true); N_C=${N_C:-0}
if [ "${N_C}" -eq "${EXPECT_C}" ]; then
  echo "PASS: C3 состав набора — ${N_C} оракулов (ожидалось ровно ${EXPECT_C}: d12 d13 d14 d15 d16 d17 d20)"
else
  echo "FAIL: C3 состав набора — ${N_C} при ожидаемых ${EXPECT_C}; порог и набор разошлись"
  FAIL=$((FAIL + 1))
fi

# ── Задача 12 (C-167): САМООПИСАНИЕ кода обязано быть правдой ──────────────────────────────
# Проверка ТЕКСТОВАЯ, и это названо честно: предмет задачи 12 — утверждение кода о себе, а
# утверждение есть текст. Барьер ловит КОНЪЮНКЦИЮ: комментарий обещает переиспользование
# уровней И при этом recompute_depth_from_book по-прежнему материализует книгу сам. Любая из
# двух развязок (реальное переиспользование ЛИБО снятие обещания) гасит шаг.
step "C4 (задача 12 — R-134 B-2(ii)) — самоописание кода не расходится с кодом"
CLAIMS=$(grep -c 'что уже читает `refresh_heatmap_bucket`' crates/gateway/src/lib.rs || true); CLAIMS=${CLAIMS:-0}
OWN=$(sed -n '/fn recompute_depth_from_book/,/^    }/p' crates/gateway/src/lib.rs | grep -c 'self\.book\.levels(' || true); OWN=${OWN:-0}
if [ "${CLAIMS}" -gt 0 ] && [ "${OWN}" -gt 0 ]; then
  echo "FAIL: C4 комментарий обещает переиспользование уровней heatmap (${CLAIMS} упом.), а recompute_depth_from_book материализует книгу сам (${OWN} вызовов self.book.levels)"
  FAIL=$((FAIL + 1))
else
  echo "PASS: C4 самоописание согласовано (обещаний=${CLAIMS}, собственных материализаций=${OWN})"
fi

# A-024 O-5: ещё два ложных самоописания (R-134 B-2(i)/(iii)). Формы показа — по R-097 N-7:
# `grep -c` при нуле печатает 0, а не молчит с exit=1, поэтому считаем ЧИСЛО, а не код возврата.
for pair in \
  'числа полосы snapshot-only|снятая snapshot-only семантика поля depth_reach_bid (lib.rs:636-658)' \
  'кадр без снимка не несёт строк|то же, вторая половина того же комментария' \
  'то же поведение, что прежний|ложное «как прежний depth_within с None mid» (lib.rs:1134-1136)'
do
  NEEDLE="${pair%%|*}"; WHY="${pair##*|}"
  N=$(grep -c "${NEEDLE}" crates/gateway/src/lib.rs || true); N=${N:-0}
  if [ "${N}" -eq 0 ]; then
    echo "PASS: C4 ложное самоописание снято — ${WHY}"
  else
    echo "FAIL: C4 ложное самоописание ЖИВО (${N} упом.) — ${WHY}"
    FAIL=$((FAIL + 1))
  fi
done

step "D (задача 9) — смена СЕМАНТИКИ объявлена bump'ом GATEWAY_SCHEMA_VERSION"
# ИМЕННО этот рычаг: `read_and_validate` шаг (3) отвергает чекпоинт при
# `gw_v != GATEWAY_SCHEMA_VERSION` (`crates/gateway/src/lib.rs:2901-2904`).
# `CKPT_SCHEMA_VERSION` — версия ФОРМАТА ФАЙЛА, формат не меняется; rev2 требовал его и дёргал
# не тот рычаг. Bump закрывает разом `C-094` B3 (явная инвалидация) и `П-014` п.3.
chk_sh "test \"\$(grep -oE 'GATEWAY_SCHEMA_VERSION: u32 = [0-9]+' crates/gateway/src/lib.rs | grep -oE '[0-9]+\$')\" -ge 9" \
       "D GATEWAY_SCHEMA_VERSION >= 9 (на момент спеки было 8)"
# `C-160` F1: bump обязан быть проведён ВМЕСТЕ с sacred-оракулом версии, иначе честная
# реализация задачи 9 роняет `cargo test --all` — первый шаг этого же гейта. Оракул прибит к 9
# в трёх публичных путях (константа, `Snapshot`, live `Frame`); шаг ниже требует его зелёным
# ОТДЕЛЬНО от агрегата, чтобы причина падения читалась сразу, а не тонула в общем выводе.
chk cargo test -p gateway --test red_gateway_schema_version --quiet

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
# `R-141` N-4: прежняя форма грепала ВЕСЬ дифф, включая КОНТЕКСТНЫЕ строки, и давала ложное
# КРАСНОЕ при любой правке compose РЯДОМ с GATEWAY_BANDS — например при объявлении соседней
# переменной (задачи 22/23). Судим только ИЗМЕНЁННЫЕ строки: `--unified=0` убирает контекст,
# фильтр `^[+-]` отбрасывает заголовки `+++`/`---`.
chk_sh "git diff --unified=0 ${BASE}..HEAD -- docker-compose.yml | grep -E '^[+-][^+-]' | grep -q 'GATEWAY_BANDS' && exit 1 || exit 0" \
       "I GATEWAY_BANDS в docker-compose.yml не тронут (судятся только изменённые строки)"

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
