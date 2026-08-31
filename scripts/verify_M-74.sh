#!/usr/bin/env bash
# Acceptance-гейт M-74 — restore-drill: копия обязана ЧИТАТЬСЯ, а не только создаваться.
#
# ГЕЙТ НАПИСАН ДО РАБОТЫ И ОБЯЗАН БЫТЬ КРАСНЫМ. Задачи 2/2b/3/4/5 открыты; шаги на них
# краснеют по построению — это RED-first, а не поломка. Зелёным гейт становится по мере
# закрытия задач; шаг, ставший зелёным РАНЬШЕ своей задачи, есть дефект гейта, и его надо
# чинить, а не радоваться.
#
# ЧЕМУ ЭТОТ ГЕЙТ НАУЧЕН ЧУЖОЙ ЦЕНОЙ — четыре урока, каждый оплачен кругом:
#   1. `R-157` `Б-5`: гейт `M-73` из семнадцати шагов НЕ ИСПОЛНЯЛ прод-конвейер ни в одном
#      шаге и потому два круга не видел мёртвого сторожа. Здесь ПОВЕДЕНИЕ проверяется
#      прогоном пробы, а не грепом по тексту обёртки.
#   2. `C-187` `B-4`: шаг, написанный ради закрытия той дыры, звал НЕСУЩЕСТВУЮЩУЮ функцию
#      `chk_sh` и молча не считался (`command not found` не увеличивает счётчик отказов).
#      Поэтому ниже стоит САМОПРОВЕРКА помощника `chk`.
#   3. `A-028` §3 п.5: `cargo test` возвращает **0 при НУЛЕ исполненных тестов**. Шаг,
#      решающий по коду возврата, зеленеет ВАКУУМНО, стоит фильтру ничего не найти. Поэтому
#      тесты гоняются через `chk_named_test` — ТРИ исхода, а не два.
#   4. `A-028` §3 п.6: `grep -q 'backup_restore_drill_ok' <файл>` зелен УЖЕ СЕГОДНЯ — он
#      ловит имя в КОММЕНТАРИИ «deferred». Проверка по ТЕКСТУ не отличает объявление от
#      вызова; поэтому канарейка эмиссии здесь привязана к ВЫЗОВУ (`set_gauge`), а исполнение
#      судит оракул `red_restore_drill_metric`.

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
ROOT="$(pwd)"

FAIL=0
step() { printf '\n── %s\n' "$*"; }
chk() {
  local name; name="$(printf '%s' "$1" | sed -n '1{s/^[[:space:]]*//;s/[[:space:]]*$//;p}')"
  [ -n "$name" ] || name="<многострочная проверка>"
  if ( eval "$1" ) >/dev/null 2>&1; then echo "PASS: ${name}"; else echo "FAIL: ${name}" >&2; FAIL=$((FAIL + 1)); fi
}

# ТРИ ИСХОДА, А НЕ ДВА (образец — `verify_M-72.sh:61-84`). Различать их обязательно:
# «оракула нет» и «оракул есть, но не собрался» — разные состояния задачи, и одинаковый
# текст отправил бы читателя искать не то. COMPILE-RED — ЗАЯВЛЕННОЕ состояние оракула
# `red_restore_drill_metric`: он написан против сигнатуры, которую вносит engine-dev
# (спека §«Сигнатура продюсера»), и до тех пор не собирается.
chk_named_test() { # $1=имя шага, далее — команда cargo
  local name="$1"; shift
  local out st ran
  out="$("$@" 2>&1)"; st=$?
  ran=$(printf '%s\n' "${out}" | awk '/^test result:/ { p += $4; f += $6 } END { print p + f + 0 }')
  if [ "${ran:-0}" -eq 0 ]; then
    if printf '%s\n' "${out}" | grep -qE 'could not compile|^error\[E[0-9]'; then
      echo "FAIL: ${name} — оракул ЕСТЬ, но НЕ СОБРАЛСЯ (COMPILE-RED): $(printf '%s\n' "${out}" | grep -m1 -E '^error' | cut -c1-100)" >&2
    else
      echo "FAIL: ${name} — НИ ОДИН тест не исполнился: фильтр не нашёл оракула. Зелёное здесь означало бы ВАКУУМ, а не закрытую задачу" >&2
    fi
    FAIL=$((FAIL + 1))
    return
  fi
  if [ ${st} -eq 0 ]; then
    echo "PASS: ${name} (исполнено тестов: ${ran})"
  else
    echo "FAIL: ${name} (исполнено тестов: ${ran}, exit=${st})" >&2
    FAIL=$((FAIL + 1))
  fi
}

# ── САМОПРОВЕРКА ПОМОЩНИКОВ. Если `chk`/`chk_named_test` окажутся не определены или
# перестанут считать отказы, ВЕСЬ гейт станет зелёным, ничего не проверив (`C-187` B-4).
_probe=0
chk "true"  >/dev/null 2>&1 || _probe=1
_before=${FAIL}
chk "false" >/dev/null 2>&1
_after_chk=${FAIL}
# `chk_named_test` на заведомо несуществующем таргете обязан дать ВАКУУМ и посчитать отказ.
chk_named_test "самопроверка вакуума" cargo test -p journal --test нет-такого-таргета --quiet >/dev/null 2>&1
if [ "${_after_chk}" -ne $((_before + 1)) ] || [ "${FAIL}" -ne $((_before + 2)) ] || [ "${_probe}" -ne 0 ]; then
  echo "FAIL: самопроверка помощников — chk или chk_named_test не считают отказы; весь гейт был бы зелёным ни о чём" >&2
  echo "VERDICT: FAIL (1)"; exit 1
fi
FAIL=${_before}
echo "PASS: самопроверка помощников — зелёное проходит, красное и ВАКУУМ считаются"

DRILL=deploy/bin/journal-restore-drill-cron.sh
EMIT=crates/recorder/src/metric_emit.rs
READER=crates/journal/src/bin/journal-drill-read.rs

step "task #0 — паритет с CI: fmt + clippy(--all-targets --all-features) + test --all"
chk "cargo fmt --all -- --check"
chk "cargo clippy --all-targets --all-features -- -D warnings"
chk "cargo test --all --quiet"

step "task #1 (RED) — фикстура ПРОД-ФОРМЫ принимается прод-читателем; drill различает исходы"
# Две половины, и обе обязательны. Первая судит саму ФИКСТУРУ прод-читателем: если её
# принимает только mock, весь остальной набор доказывает не то (`A-028` §3 п.2). Вторая —
# поведение обёртки на этой фикстуре.
chk_named_test "фикстура прод-формы читается journal::stream" \
  cargo test -p journal --test fixture_restore_drill_cold --quiet
chk "bash scripts/tests/red_restore_drill.sh"

step "task #2 — обёртка drill'а существует и исполняется ПРОД-ФОРМОЙ вызова"
chk "test -x ${DRILL}"
# Восстановление НЕ СМЕЕТ идти в боевой каталог: проверка бэкапа не должна становиться
# риском для оригинала (§Запрещено).
chk "! grep -q 'hft-platform_journal-data/_data' ${DRILL}"
# Композиция путей: обёртка обязана ПЕЧАТАТЬ argv до побочных эффектов, и печать обязана
# называть тот же каталог для записи доставки и для чтения читателем. Рассогласование двух
# строк даёт тихий no-op — класс `verify_M-48`.
chk "HFT_CRON_PRINT_ARGV=1 bash ${DRILL} | grep -q '^RESTORE_DIR='"
# ⚠ НЕПУСТОТА ОБЯЗАТЕЛЬНА. Первая редакция этого шага сравнивала два `$(...)` напрямую и
# была ВАКУУМНО ЗЕЛЁНОЙ, пока обёртки нет: `"" = ""` истинно. Поймано первым же прогоном
# гейта — ровно тот класс, ради которого рядом стоит `chk_named_test`.
chk "w=\$(HFT_CRON_PRINT_ARGV=1 bash ${DRILL} 2>/dev/null | sed -n 's/^RESTORE_DIR=//p'); r=\$(HFT_CRON_PRINT_ARGV=1 bash ${DRILL} 2>/dev/null | sed -n 's/^READER_DIR=//p'); [ -n \"\$w\" ] && [ \"\$w\" = \"\$r\" ]"

step "task #2b — читатель drill'а существует и РАЗЛИЧАЕТ коды отказа"
chk "test -f ${READER}"
# Коды 4 (ЧТЕНИЕ) / 5 (ПУСТОТА) / 6 (КОНТЕКСТ) — машинный контракт, на котором стоит
# различение причин в файле состояния. Проверяется по ВЫЗОВУ: пустой каталог обязан дать 5.
chk "d=\$(mktemp -d) && cargo run -q -p journal --bin journal-drill-read -- --dir \$d --min-events 1; rc=\$?; rm -rf \$d; [ \$rc -eq 5 ]"
# Бинарь обязан попасть в ПРОД-ОБРАЗ, иначе обёртка на VPS его не найдёт: файл в репозитории
# при деплое, который его не устанавливает, инертен (`testing.md` §«Механизм несущего пути»).
chk "grep -q 'journal-drill-read' Dockerfile"

step "task #3 — метрика РЕАЛЬНО эмитится, а не только объявлена (OPS-I-10)"
# Канарейка привязана к ВЫЗОВУ, а не к имени: `grep 'backup_restore_drill_ok'` зелен уже
# сегодня, потому что имя стоит в комментарии «deferred» (`A-028` §3 п.6).
chk "grep -qE 'set_gauge\(\s*\"backup_restore_drill_ok\"' ${EMIT}"
chk "! grep -qE 'backup_restore_drill_ok.*deferred' ${EMIT}"
chk_named_test "отображение «файл состояния → gauge в рендере /metrics»" \
  cargo test -p recorder --test red_restore_drill_metric --quiet

step "task #4 — расписание drill'а принимается ПРОД-ПАРСЕРОМ"
if ! command -v crontab >/dev/null 2>&1; then
  echo "FAIL: crontab недоступен — синтаксис расписания НЕ ПРОВЕРЕН; это отказ СРЕДЫ, а не зелёный гейт" >&2
  FAIL=$((FAIL + 1))
else
  chk "test -f deploy/cron.d/journal-restore-drill"
  chk "crontab -n deploy/cron.d/journal-restore-drill"
fi

step "task #5 — ПРОСРОЧЕННОСТЬ есть отказ: молчание не считается успехом"
chk_named_test "просроченный успешный drill ⇒ метрика 0 (и внутри окна ⇒ 1)" \
  cargo test -p recorder --test red_restore_drill_metric --quiet stale

step "C — граница C не тронута: RETENTION_MODE остаётся dry-run"
# Милестоун ОТКРЫВАЕТ вопрос о включении удаления, но не решает его. Подпись founder'а
# не заменяется прохождением гейта (`П-023`, порядок «копия → восстановление → удаление»).
BASE=$(git merge-base HEAD origin/main 2>/dev/null || echo "")
if [ -z "${BASE}" ]; then
  echo "FAIL: merge-base с origin/main не вычислен — шаги диапазона судить не по чему" >&2
  FAIL=$((FAIL + 1))
else
  chk "git diff ${BASE}..HEAD -- docker-compose.yml | grep -qE '^[+-].*RETENTION_MODE' && exit 1 || exit 0"
  # `crates/journal/**` тронут ТОЛЬКО двумя разрешёнными путями: новым бинарём-читателем и
  # тест-таргетом фикстуры. Логика журнала ради проверки его копии не правится (§Forbidden).
  chk "git diff --name-only ${BASE}..HEAD -- crates/journal crates/contracts | grep -vE '^crates/journal/(src/bin/journal-drill-read\.rs|tests/fixture_restore_drill_cold\.rs)$' | grep -q . && exit 1 || exit 0"
fi

echo
if [ "${FAIL}" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (${FAIL})"; exit 1
