#!/usr/bin/env bash
# Acceptance-гейт M-72 — терминальность подписки: одно свойство, три носителя.
#
# ПРЕДМЕТ (замер, не память; ревизия ветки d8c7654, `crates/gateway-serve/src/lib.rs`):
#   · сохранение подписки СВЕРЯЕТ поколение — `:1306`, `:1391`, `:1740`, `:1796`
#     (`!drained && current_gen == Some(gen_at_pump)`);
#   · снятие подписки по терминальному отказу поколение НЕ сверяет — `:1405`, `:1807`
#     (`subs.remove` под `if cap_terminal`). Это `TD-177`: отставший `pump` старого селектора
#     сносит подписку, созданную ПОЗЖЕ.
#
# ПЕРЕПИСЬ НОСИТЕЛЕЙ ВЫПИСАНА, а не подразумевается (`Р-3`: опасна группа, которая НЕ
# выписана). `subs.remove` встречается ТРИЖДЫ, и три — разные вещи:
#   :1019  штатный обработчик `unsubscribe` — НЕ носитель дефекта, снятие здесь запрошено клиентом;
#   :1405  v1, ветка сообщения      — носитель 1;
#   :1807  v1, ветка периодического тика — носитель 2.
# Пятый `if cap_terminal` (`:1874`) носителем НЕ является и в перепись не входит: на
# legacy-пути подписка ОДНА и совпадает с соединением, `subs`/`gens` там нет вовсе,
# терминальность = `ServeMsg::Error` + закрытие. Разница названа здесь, чтобы следующий круг
# не переоткрывал её и не «чинил» то, у чего нет поколения.
# Шаг `P` ниже СЧИТАЕТ эту перепись: появление третьего носителя красит гейт.
#
# ГЕЙТ НАПИСАН ДО РАБОТЫ И ОБЯЗАН БЫТЬ КРАСНЫМ. Задачи 2, 3, 5, 6, 8 открыты; шаги на них
# краснеют по построению — это RED-first, а не поломка. Зелёным гейт становится по мере
# закрытия задач; шаг, ставший зелёным раньше своей задачи, есть дефект гейта.

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
# Корень вычисляется ОДИН раз: ниже есть подоболочки с `cd`, где относительный `$0` уже врёт.
ROOT="$(pwd)"

FAIL=0
step() { echo "=== $* ==="; }
chk() { if "$@"; then echo "PASS: $*"; else echo "FAIL: $*"; FAIL=$((FAIL + 1)); fi; }
chk_sh() { if bash -c "$1" >/dev/null 2>&1; then echo "PASS: $2"; else echo "FAIL: $2"; FAIL=$((FAIL + 1)); fi; }
# Состав набора СЧИТАЕТСЯ, а не заявляется. Регексп ОБЯЗАН брать цифры в именах: на `M-68`
# класс `[a-z_]` не взял `d18e`/`d18f` и дал ложное КРАСНОЕ на ПРАВИЛЬНОЙ правке.
count_fns() { grep -cE "$1" "$2" 2>/dev/null || echo 0; }
expect_count() { # $1=имя шага $2=факт $3=ожидание $4=расшифровка
  if [ "$2" -eq "$3" ]; then
    echo "PASS: $1 состав набора — $2 (ожидалось ровно $3: $4)"
  else
    echo "FAIL: $1 состав набора — $2 при ожидаемых $3 ($4); порог и набор разошлись"
    FAIL=$((FAIL + 1))
  fi
}

SERVE=crates/gateway-serve/src/lib.rs
T1=crates/gateway-serve/tests/red_ws_terminality_entrypoint.rs
T4=crates/gateway/tests/red_pump_midstream_failure.rs
T6=crates/gateway/tests/red_snapshot_cursor_honesty.rs

BASE=$(git merge-base HEAD origin/main 2>/dev/null || echo "")
if [ -z "${BASE}" ]; then
  echo "FAIL: merge-base с origin/main не вычислен — шаги диапазона судить не по чему"
  FAIL=$((FAIL + 1))
fi

step "task #0 — паритет с CI: fmt + clippy(--all-targets --all-features) + test --all"
chk cargo fmt --all -- --check
chk cargo clippy --all-targets --all-features -- -D warnings
chk cargo test --all --quiet

step "task #1 (TD-178) — оракул ТОЧКИ ВХОДА: прод-форма вызова, живая WS-сессия"
chk cargo test -p gateway-serve --test red_ws_terminality_entrypoint --quiet
expect_count "1" "$(count_fns '^async fn td_17[0-9]_e[0-9]+_[a-z0-9_]+\(\)' "$T1")" 2 "E-1 vantage + E-2 предмет"
# Оракул точки входа обязан ИСПОЛНЯТЬ границу процесса, а не описывать её. Признак —
# реальный сокет: `grep` по имени поймал бы и лог-строку (testing.md, канарейка по ВЫЗОВУ).
chk_sh "grep -q 'connect_async\|TcpListener' ${T1}" \
       "1 оракул поднимает реальную WS-сессию, а не зовёт библиотеку"

step "task #2 (TD-177) — RED: отставший pump не убивает НОВУЮ подписку"
# Открыт. Красное здесь — ожидаемое состояние RED-first, а не поломка гейта.
chk cargo test -p gateway-serve --features testing --test red_ws_terminality_entrypoint \
    td177_stale_pump_does_not_kill_new_sub --quiet

step "task #3 (TD-177) — фикс: снятие сверяет поколение теми же условиями, что и сохранение"
# Носителей ДВА, и проверяются ОБА. Сверка по одному оставила бы второй молча дефектным —
# ровно класс Р-3, стоивший кругов на соседнем предмете.
N_SAVE=$(count_fns 'current_gen == Some\(gen_at_pump\)' "$SERVE")
if [ "${N_SAVE}" -ge 4 ]; then
  echo "PASS: 3 условие сохранения на месте — ${N_SAVE} вхождений сверки поколения"
else
  echo "FAIL: 3 условие сохранения исчезло (${N_SAVE} вхождений) — фикс куплен ценой соседнего инварианта"
  FAIL=$((FAIL + 1))
fi
# ФИКС ПРЕДЪЯВЛЯЕТСЯ ТЕМ, ЧТО СНЯТИЕ СТОИТ ПОД СВЕРКОЙ ПОКОЛЕНИЯ. Пока задача 3 открыта,
# `subs.remove` под `cap_terminal` поколение не сверяет, и шаг красен по построению.
REMOVE_UNGUARDED=$(awk '
  /if cap_terminal \{/ { win = 12; guarded = 0 }
  win > 0 && /current_gen == Some\(gen_at_pump\)/ { guarded = 1 }
  win > 0 && /\.subs\.remove\(/ && !guarded { print NR }
  win > 0 { win-- }
' "$SERVE" | wc -l)
if [ "${REMOVE_UNGUARDED}" -eq 0 ]; then
  echo "PASS: 3 снятие подписки под cap_terminal сверяет поколение на всех носителях"
else
  echo "FAIL: 3 снятие подписки без сверки поколения — носителей ${REMOVE_UNGUARDED} (TD-177 жив)"
  FAIL=$((FAIL + 1))
fi

step "task #4 (TD-179) — RED: закладка доставки не уходит дальше доставленного"
chk cargo test -p gateway --test red_pump_midstream_failure --quiet
expect_count "4" "$(count_fns '^fn td_17[0-9]_m[0-9]+_[a-z0-9_]+\(\)' "$T4")" 2 "M-1 позитивный контроль + M-2 предмет"

step "task #5 (TD-179) — фикс по выбранной форме извещения"
# Задача 5 не имеет СВОЕГО теста: её предъявляет ЗЕЛЁНЫЙ набор задачи 4 (то же требование,
# что «задача 2 GREEN» у задачи 3). Отдельная проверка здесь была бы бухгалтерией.
chk_sh "cargo test -p gateway --test red_pump_midstream_failure --quiet >/dev/null 2>&1" \
       "5 набор задачи 4 ЗЕЛЁН — форма извещения выбрана и реализована"

step "task #6 (TD-180) — честность курсора в snapshot()"
chk_sh "test -f ${T6}" "6 оракул честности курсора существует"
chk cargo test -p gateway --test red_snapshot_cursor_honesty --quiet

step "task #7 — приземлённый P7 цел, состав набора предела не тронут"
chk cargo test -p gateway --test red_egress_cap_paths --quiet
expect_count "7" "$(count_fns '^fn pl_i_5_p[a-z0-9_]*\(\)' crates/gateway/tests/red_egress_cap_paths.rs)" 8 "P-C1 P1 P2 P3 P4 P5 P6 P7"

step "task #8 — ПУТЬ гейта M-71 снят из чужой зоны (иначе гейт не переезжает в архив)"
N_REF=$(git grep -c 'scripts/verify_M-71' -- crates/ deploy/ 2>/dev/null | wc -l)
if [ "${N_REF}" -eq 0 ]; then
  echo "PASS: 8 ссылок на scripts/verify_M-71 в crates//deploy/ нет"
else
  echo "FAIL: 8 ссылка на scripts/verify_M-71 жива в чужой зоне (файлов: ${N_REF})"
  FAIL=$((FAIL + 1))
fi
# Якорь мутации ОСТАЁТСЯ — снимается только литеральный путь. Проверяется ОТДЕЛЬНО, иначе
# «снял ссылку» и «снёс якорь заодно» неразличимы.
chk_sh "grep -q 'MUT-ANCHOR M-71-LIMIT' crates/gateway/src/lib.rs" \
       "8 якорь MUT-ANCHOR M-71-LIMIT на месте (снимался только путь)"

step "P (Р-3) — ПЕРЕПИСЬ НОСИТЕЛЕЙ: появление третьего сайта снятия обязано краснеть"
expect_count "P" "$(count_fns '\.subs\.remove\(' "$SERVE")" 3 ":1019 unsubscribe + два носителя под cap_terminal"
expect_count "P" "$(count_fns 'if cap_terminal \{' "$SERVE")" 5 "два v1-носителя + два вспомогательных + legacy-завершение"

step "S — тестовый шов виден ТОЛЬКО под feature=testing; в прод-сборке его не существует"
chk_sh "grep -q '^testing = \[\]' crates/gateway-serve/Cargo.toml" "S фича testing объявлена"
# Шов обязан стоять под cfg. Проверка ОТСУТСТВИЯ: если имя шва встречается вне cfg-гейта —
# он попал в прод-сборку, и оракул судит не тот код.
SEAM_UNGATED=$(awk '
  /#\[cfg\(feature = "testing"\)\]/ { gated = 3 }
  /pump_gate|pump_started|test_seam/ && gated == 0 { print NR }
  gated > 0 { gated-- }
' "$SERVE" | wc -l)
if [ "${SEAM_UNGATED}" -eq 0 ]; then
  echo "PASS: S шва вне cfg(feature=\"testing\") нет — прод-путь не содержит тестовых ветвлений"
else
  echo "FAIL: S шов встречается вне cfg(feature=\"testing\") — строк: ${SEAM_UNGATED}"
  FAIL=$((FAIL + 1))
fi
# Прод-сборка обязана собираться БЕЗ фичи — иначе шов стал обязательным.
chk cargo build -p gateway-serve --quiet

step "M — МУТАЦИОННЫЙ КОНТРОЛЬ задачи 1: нейтрализация терминальности роняет E-2, E-1 цел"
# Мутация идёт в ИЗОЛИРОВАННОЙ копии — гейт не смеет править рабочее дерево.
#
# КЭШ СБОРКИ У МУТАЦИИ СВОЙ, И ЭТО НЕ ОПТИМИЗАЦИЯ, А КОРРЕКТНОСТЬ. Первая редакция шага
# делила `${ROOT}/target` с чистым деревом — «иначе шаг стоит десяти минут». Это ОТРАВЛЯЕТ
# кэш: артефакт, собранный из МУТИРОВАННОГО исходника, остаётся в общем каталоге, и
# следующий прогон в ЧИСТОМ дереве исполняет его. Поймано на себе тем же вечером:
#
#   до пробы фикса   : td_180_s2 ... FAILED   (правильно — дефект есть)
#   после пробы фикса: td_180_s2 ... ok       ← ЛОЖНОЕ ЗЕЛЁНОЕ; `cargo` не пересобирал
#                                               («Finished in 0.04s»), исходник при этом
#                                               НЕ чинён (`cursor: self.cursor,` на месте)
#   после `touch` исходника: td_180_s2 ... FAILED   (снова правда)
#
# То есть общий кэш делает РЕЗУЛЬТАТ ГЕЙТА зависящим от того, что прогоняли перед ним, —
# ровно свойство 2 `testing.md` (оракул обязан мерить свой инвариант, а не окружение).
# Отдельный `target-mutation/` платит полной сборкой ОДИН раз; дальше он тёплый, а чистое
# дерево не трогается никогда.
# BUILD_EXIT предъявляется ОТДЕЛЬНО от результата теста: несобравшаяся мутация даёт «тест не
# прошёл» и была бы засчитана за срабатывание оракула (замер 28.08 — ровно этот случай).
if [ "${HFT_M72_MUTATION:-1}" = "0" ]; then
  echo "FAIL: M мутационный контроль ОТКЛЮЧЁН (HFT_M72_MUTATION=0) — гейт без него не полон"
  FAIL=$((FAIL + 1))
else
  MUT=$(mktemp -d /tmp/m72-mut-XXXXXX)
  trap 'rm -rf "${MUT}"' EXIT
  git ls-files -z | xargs -0 -I{} sh -c 'mkdir -p "'"${MUT}"'/$(dirname {})" && cp {} "'"${MUT}"'/{}"' 2>/dev/null
  if ! cp -r .git "${MUT}/.git" 2>/dev/null; then :; fi
  # Мутация целит в КОД, а не в комментарий: терминальность объявляется невозможной.
  sed -i 's/let cap_terminal = live\.is_cap_terminal();/let cap_terminal = false;/g' \
    "${MUT}/${SERVE}"
  if cmp -s "${SERVE}" "${MUT}/${SERVE}"; then
    echo "FAIL: M мутация НЕ ВНЕСЛАСЬ — якорь не найден; шаг засчитал бы отсутствие правки за срабатывание"
    FAIL=$((FAIL + 1))
  else
    ( cd "${MUT}" && CARGO_TARGET_DIR="${ROOT}/target-mutation" \
        cargo build -p gateway-serve --tests --quiet ) >/dev/null 2>&1
    BUILD_EXIT=$?
    if [ ${BUILD_EXIT} -ne 0 ]; then
      echo "FAIL: M мутация НЕ СОБРАЛАСЬ (BUILD_EXIT=${BUILD_EXIT}) — «тест упал» здесь означает отказ сборки, а не срабатывание оракула"
      FAIL=$((FAIL + 1))
    else
      ( cd "${MUT}" && CARGO_TARGET_DIR="${ROOT}/target-mutation" \
          cargo test -p gateway-serve --test red_ws_terminality_entrypoint \
          td_178_e2 --quiet ) >/dev/null 2>&1
      E2=$?
      ( cd "${MUT}" && CARGO_TARGET_DIR="${ROOT}/target-mutation" \
          cargo test -p gateway-serve --test red_ws_terminality_entrypoint \
          td_178_e1 --quiet ) >/dev/null 2>&1
      E1=$?
      if [ ${E2} -ne 0 ] && [ ${E1} -eq 0 ]; then
        echo "PASS: M нейтрализация терминальности → E-2 FAILED (exit=${E2}), E-1 цел (exit=${E1}); BUILD_EXIT=0"
      else
        echo "FAIL: M мутация не различила оракулы — E-2 exit=${E2} (ожидался ≠0), E-1 exit=${E1} (ожидался 0); BUILD_EXIT=0"
        FAIL=$((FAIL + 1))
      fi
    fi
  fi
fi

step "C — Block-C: contracts предметом не тронуты"
chk_sh "git diff --name-only ${BASE}..HEAD -- crates/contracts | grep -q . && exit 1 || exit 0" \
       "C crates/contracts не тронут"

step "G — состав ВЫДАЧИ не тронут: включение полос есть граница C и предмет M-70"
chk_sh "git diff ${BASE}..HEAD -- docker-compose.yml | grep -qE '^[+-].*GATEWAY_BANDS' && exit 1 || exit 0" \
       "G GATEWAY_BANDS не тронут"

step "H — зона предмета: чужие крейты в диапазоне не участвуют"
chk_sh "git diff --name-only ${BASE}..HEAD -- crates/book crates/venue-binance crates/venue-binance-futures crates/journal crates/contracts | grep -q . && exit 1 || exit 0" \
       "H book/venue/journal/contracts не тронуты диапазоном"

echo
if [ "${FAIL}" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (${FAIL})"; exit 1
