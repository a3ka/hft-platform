#!/usr/bin/env bash
# Acceptance-гейт M-74 — restore-drill: копия обязана ЧИТАТЬСЯ, а не только создаваться.
#
# ГЕЙТ НАПИСАН ДО РАБОТЫ И ОБЯЗАН БЫТЬ КРАСНЫМ. Все шесть задач открыты; шаги на них
# краснеют по построению — это RED-first, а не поломка. Зелёным гейт становится по мере
# закрытия задач; шаг, ставший зелёным РАНЬШЕ своей задачи, есть дефект гейта, и его надо
# чинить, а не радоваться.
#
# ЧЕМУ ЭТОТ ГЕЙТ НАУЧЕН ЧУЖОЙ ЦЕНОЙ (`R-157` `Б-5`, `C-187` `B-4`) — два урока соседнего
# милестоуна, оба оплачены кругами:
#   1. Гейт `M-73` из семнадцати шагов НЕ ИСПОЛНЯЛ прод-конвейер ни в одном шаге и потому
#      два круга не видел мёртвого сторожа. Здесь ПОВЕДЕНИЕ проверяется прогоном пробы,
#      а не грепом по тексту обёртки.
#   2. Шаг, написанный ради закрытия той дыры, звал НЕСУЩЕСТВУЮЩУЮ функцию `chk_sh` и
#      молча не считался: `command not found` не увеличивает счётчик отказов. Поэтому
#      здесь ОДИН помощник `chk`, и ниже стоит его самопроверка.

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

# САМОПРОВЕРКА ПОМОЩНИКА. Прямое следствие `C-187` B-4: если `chk` окажется не определён
# или перестанет считать отказы, ВЕСЬ гейт станет зелёным, ничего не проверив. Дешевле
# одной строки — убедиться, что он умеет и то, и другое.
_probe=0
chk "true"  >/dev/null 2>&1 || _probe=1
_before=${FAIL}
chk "false" >/dev/null 2>&1
if [ "${FAIL}" -ne $((_before + 1)) ] || [ "${_probe}" -ne 0 ]; then
  echo "FAIL: самопроверка chk — помощник не считает отказы; весь гейт был бы зелёным ни о чём" >&2
  echo "VERDICT: FAIL (1)"; exit 1
fi
FAIL=${_before}
echo "PASS: самопроверка chk — зелёное проходит, красное СЧИТАЕТСЯ"

DRILL=deploy/bin/journal-restore-drill-cron.sh
EMIT=crates/recorder/src/metric_emit.rs

step "task #0 — паритет с CI: fmt + clippy(--all-targets --all-features) + test --all"
chk "cargo fmt --all -- --check"
chk "cargo clippy --all-targets --all-features -- -D warnings"
chk "cargo test --all --quiet"

step "task #1 (RED) — drill различает НЕЧИТАЕМОСТЬ и ОТСУТСТВИЕ КОНТЕКСТА"
# Проба сама несёт позитивный контроль (H) и мутацию различения — см. её шапку.
chk "bash scripts/tests/red_restore_drill.sh"

step "task #2 — обёртка drill'а существует и исполняется прод-формой"
chk "test -x ${DRILL}"
# Восстановление НЕ СМЕЕТ идти в боевой каталог: проверка бэкапа не должна становиться
# риском для оригинала (§Запрещено). Ловится грепом по литералу боевого пути.
chk "! grep -q 'hft-platform_journal-data/_data' ${DRILL}"

step "task #3 — метрика РЕАЛЬНО эмитится, а не только объявлена (OPS-I-10)"
# Продюсер — sampler recorder'а: `/metrics` держит Arc<Metrics> внутри его процесса,
# внешняя обёртка поставить gauge туда не может (`C-187` B-2). Значит эмиссия обязана
# жить ЗДЕСЬ, и «deferred» в продюсер-карте обязано исчезнуть.
chk "grep -q 'backup_restore_drill_ok' ${EMIT}"
chk "! grep -qE 'backup_restore_drill_ok.*deferred' ${EMIT}"
chk "cargo test -p recorder --test red_restore_drill_metric --quiet"

step "task #4 — расписание drill'а принимается ПРОД-ПАРСЕРОМ"
if ! command -v crontab >/dev/null 2>&1; then
  echo "FAIL: crontab недоступен — синтаксис расписания НЕ ПРОВЕРЕН; это отказ СРЕДЫ, а не зелёный гейт" >&2
  FAIL=$((FAIL + 1))
else
  chk "test -f deploy/cron.d/journal-restore-drill"
  chk "crontab -n deploy/cron.d/journal-restore-drill"
fi

step "task #5 — ПРОСРОЧЕННОСТЬ есть отказ: молчание не считается успехом"
chk "cargo test -p recorder --test red_restore_drill_metric --quiet stale"

step "C — граница C не тронута: RETENTION_MODE остаётся dry-run"
# Милестоун ОТКРЫВАЕТ вопрос о включении удаления, но не решает его. Подпись founder'а
# не заменяется прохождением гейта (`П-023`, порядок «копия → восстановление → удаление»).
BASE=$(git merge-base HEAD origin/main 2>/dev/null || echo "")
if [ -z "${BASE}" ]; then
  echo "FAIL: merge-base с origin/main не вычислен — шаги диапазона судить не по чему" >&2
  FAIL=$((FAIL + 1))
else
  chk "git diff ${BASE}..HEAD -- docker-compose.yml | grep -qE '^[+-].*RETENTION_MODE' && exit 1 || exit 0"
  chk "git diff --name-only ${BASE}..HEAD -- crates/journal crates/contracts | grep -q . && exit 1 || exit 0"
fi

echo
if [ "${FAIL}" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL (${FAIL})"; exit 1
