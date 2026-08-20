#!/usr/bin/env bash
# RED-проба M-66: reviewer verdict должен предъявлять живое FA-эхо тронутого модуля.
#
# ОЖИДАЕМОЕ СОСТОЯНИЕ СЕЙЧАС: КРАСНОЕ. `scripts/check_review_fa.sh` ещё не существует
# (это задача 2 dev-цикла), поэтому обычный прогон обязан закончиться строкой
# `SETUP НЕ СОСТОЯЛСЯ: барьера нет ... 127 от bash неотличим от честного отказа гейта`
# и НЕНУЛЕВЫМ exit-кодом. Это не дефект пробы: RED обязан жить до реализации барьера.
#
# Источник состава — таблица §4 `milestones/M-66-protocol-attestation.md`: 8 осей,
# 35 сценарных строк. Число в финальном `VERDICT: PASS (N/N)` считается из манифеста,
# а не пишется литералом. Комбинированные строки оси 5 (`E5EMPTY · E5ALIEN` и т.п.)
# исполняют каждую названную подформу внутри одной сценарной строки, как в утверждённой
# таблице, поэтому знаменатель остаётся равен табличному составу.
#
# Setup-guard обязателен на КАЖДЫЙ сценарий: пустой диапазон, неслучившийся commit или
# отсутствующий барьер не засчитываются как честный отказ проверяемого гейта.
#
# Герметичность к окружению: проба снимает ambient git identity. Каждый fixture-коммит
# несёт `-c user.name=... -c user.email=...` явно; переменные идентичности не экспортируются
# в барьер.

set -uo pipefail

SELF="$(cd "$(dirname "$0")" && pwd)/$(basename "$0")"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER:-${ROOT}/scripts/check_review_fa.sh}"
SPEC="${SPEC:-${ROOT}/milestones/M-66-protocol-attestation.md}"
CI="${CI:-${ROOT}/.github/workflows/ci.yml}"
ZERO=0000000000000000000000000000000000000000

FAILED=0
PASSED=0
EXECUTED=""
LAST_RC=0
LAST_OUT=""

FIXTURES_REG="$(mktemp /tmp/red-review-fa-reg-XXXXXX)" || {
  echo "SETUP НЕ СОСТОЯЛСЯ: не создан реестр фикстур" >&2
  exit 2
}
SANDBOX_HOME="$(mktemp -d /tmp/red-review-fa-home-XXXXXX)" || {
  echo "SETUP НЕ СОСТОЯЛСЯ: не создан HOME-песочник" >&2
  exit 2
}
printf '%s\n' "${SANDBOX_HOME}" >> "${FIXTURES_REG}"

export HOME="${SANDBOX_HOME}"
export GIT_CONFIG_GLOBAL=/dev/null
export GIT_CONFIG_SYSTEM=/dev/null
unset GIT_AUTHOR_NAME GIT_AUTHOR_EMAIL GIT_COMMITTER_NAME GIT_COMMITTER_EMAIL EMAIL

MANIFEST="$(cat <<'EOF_MANIFEST'
D1DOC|1 состав диапазона|doc-only (без `crates/**`)|D1DOC|SKIP, exit 0 — барьер МОЛЧИТ
D1NOREV|1|`crates/**` тронут, S=∅|D1NOREV|FAIL (механизм D)
D1REVPAIR|1|реверт-пара, net-diff чист|D1REVPAIR|SKIP, exit 0 (per-range семантика §3.1)
D1NOFA|1|только NO-FA крейт (`recorder`), R введён БЕЗ `FA-WAIVER`|D1NOFA|**FAIL** (пробел покрытия — §2 W; rev1 ставила PASS, опровергнуто C-082 B-1)
D1NOFAW|1|только NO-FA крейт, R с `FA-WAIVER: crates/recorder — <причина>`|D1NOFAW|PASS + печать `WAIVED` (легитимный оси W)
D1NOFANOREV|1|только NO-FA крейт, S=∅|D1NOFANOREV|FAIL (D действует и на NO-FA)
M2KNOWN|2 маппинг|известный FA-крейт|(легитимный B4LIVE)|—
M2UNKNOWN|2|незнакомое имя (`crates/journal2` — rename/новый)|M2UNKNOWN|FAIL
M2VENUE|2|`venue-*` glob (`crates/venue-xyz`)|M2VENUE|PASS при живом `VN-I-*`
S3ADDED|3 источник S|R введён диапазоном|(легитимный B4LIVE)|—
S3NAMED|3|R назван полным путём в `%B`, существует на HEAD|S3NAMED|PASS при живом эхе
S3GHOST|3|назван путь, файла на HEAD нет (ghost)|S3GHOST|не входит в S → FAIL
S3BAREID|3|голый `R-NNN` в subject без пути|S3BAREID|не входит в S → FAIL
B4LIVE|4 эхо|живой ID своего префикса|B4LIVE|PASS (печать `файл: ID`)
B4CROSSCUT|4|только сквозной ID, цитируемый в FA (`DET-I-1`)|B4CROSSCUT|FAIL (запиннено R-053, §0 стр. 4/13)
B4FOREIGN|4|ID чужой FA (`VB-I-*` при дифе только `journal`)|B4FOREIGN|FAIL
B4DEAD|4|синтаксически валидный, но МЁРТВЫЙ ID (`JR-I-999`)|B4DEAD|FAIL
B4UNION|4|несколько крейтов, живой ID ОДНОГО из них|B4UNION|PASS (запиннено R-040, §0 стр. 7/9/12)
B4DEADPFX|4|мёртвый ID, содержащий ЖИВОЙ как префикс (`JR-I-14` при живых `JR-I-1..13`)|B4DEADPFX|FAIL (эхо словоцелое; `grep -F` даёт ложное PASS — R-079 Б-1)
B4LIVETAIL|4|живой ID-хвост набора (`JR-I-13`), сам содержащий живой `JR-I-1`|B4LIVETAIL|PASS (анти-плацебо к слишком строгой реализации)
B4EMPTYU|4|FA-файл есть, но НЕ несёт ни одного ID своего префикса (U=∅ при непустом LIVE_CRA)|B4EMPTYU|FAIL (тихий отказ гейта — R-079 Б-2)
E5EVENT|5 событие/база|`EVENT_NAME` пуст · неизвестен|E5EMPTY · E5ALIEN|FAIL каждый
E5BASE|5|база пуста · zero-SHA · не в истории · не предок HEAD|E5BASE0 · E5ZERO · E5LOST · E5NONANC|FAIL каждый
E5OK|5|легитимный push и легитимный PR|E5PUSH · E5PR|PASS (та же фикстура B4LIVE двумя событиями)
F6NOFAFILE|6 setup|FA-файл маппинга отсутствует на HEAD|F6NOFAFILE|FAIL (не «пустое множество»)
F6NODIR|6|каталога `docs/fa/` нет вовсе|F6NODIR|FAIL
F6NOREV|6|каталога `research/reviews/` нет|F6NOREV|S=∅ → FAIL (через D)
F6SETUPGUARD|6|setup-guard: сломанная фикстура сценария|— у КАЖДОГО сценария|проба валится САМА, не тестирует не то (`testing.md` §4 св-во 3)
W7WRONG|7 waiver|waiver называет НЕ тронутый крейт (`FA-WAIVER: crates/derive` при дифе `recorder`)|W7WRONG|FAIL (waiver — не токен на предъявителя)
W7MIXGAP|7|смешанный диф `journal`+`recorder`, живое `JR`-эхо, waiver нет|W7MIXGAP|FAIL (W per-crate: эхо соседа пробел не гасит)
W7MIXW|7|то же + `FA-WAIVER: crates/recorder`|W7MIXW|PASS (эхо B + предъявленный пробел W — легитимный)
W7EMPTY|7|waiver с ПУСТОЙ причиной: `FA-WAIVER: crates/recorder — `|W7EMPTY|**FAIL** (`C-085` B-1: порог был объявлен и не запиннен)
W7SHORT|7|причина на ОДИН символ короче порога (11)|W7SHORT|**FAIL** (нижняя сторона границы)
W7EXACT|7|причина РОВНО в порог (12)|W7EXACT|**PASS** (верхняя сторона границы)
W7PFX|7|waiver называет крейт-суффикс тронутого (`crates/recorder2` при дифе `recorder`)|W7PFX|FAIL (предикат W якорный, а не подстрочный)
W8NONEEDS|8 проводка CI|джоб `review-fa` отсутствует в `needs` `status-check`|W8NONEEDS|wiring-секция пробы FAIL
W8NOIF|8|джоб есть в `needs`, но нет в условии `if` шага агрегата|W8NOIF|FAIL (вторая рукописная копия списка — §0 стр. 21)
W8NOCALL|8|джоб не зовёт барьер или не зовёт пробу|W8NOCALL|FAIL
W8OK|8|полная проводка (реальный `ci.yml` чекаута)|W8OK|PASS — легитимный, гоняется КАЖДЫМ прогоном пробы в CI
W8CMTONLY|8|`fetch-depth: 0` остался ТОЛЬКО в комментарии, исполняемая директива снята|W8CMTONLY|FAIL (игла по тексту ловилась на комментарии — R-081 N-5)
EOF_MANIFEST
)"

cleanup() {
  if [ "${KEEP_FIXTURES:-0}" = "1" ]; then
    echo "фикстуры сохранены: ${FIXTURES_REG}" >&2
    return
  fi
  local d
  while IFS= read -r d; do
    [ -n "${d}" ] && [ -d "${d}" ] && case "${d}" in
      /tmp/red-review-fa-*) rm -rf "${d}" ;;
    esac
  done < "${FIXTURES_REG}"
  rm -f "${FIXTURES_REG}"
}
trap cleanup EXIT

die() {
  echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2
  exit 2
}

mark() {
  EXECUTED="${EXECUTED}$1
"
}

pass_scenario() {
  echo "PASS  $*"
  PASSED=$((PASSED + 1))
}

fail_scenario() {
  echo "FAIL  $*"
  FAILED=$((FAILED + 1))
}

meta_pass() { echo "PASS  $*"; }
meta_fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

require_barrier() {
  local scenario="$1"
  [ -f "${BARRIER}" ] || die "${scenario}: барьера нет: ${BARRIER}. Проба НЕ имеет права выносить вердикт: 127 от bash неотличим от честного отказа гейта."
  bash -n "${BARRIER}" 2>/dev/null || die "${scenario}: барьер не парсится — его отказ неотличим от ошибки интерпретатора."
}

register_dir() {
  printf '%s\n' "$1" >> "${FIXTURES_REG}"
}

mk_repo() {
  local name="$1" d
  d="$(mktemp -d "/tmp/red-review-fa-${name}-XXXXXX")" || die "${name}: mktemp"
  register_dir "${d}"
  (
    cd "${d}" || exit 1
    git init -q -b main
    mkdir -p docs/fa research/reviews crates/journal/src crates/gateway/src \
      crates/venue-xyz/src crates/recorder/src docs
    # ПРОД-ФОРМА живого множества (`testing.md` §«Форма прода снимается ЗАМЕРОМ»).
    # Замер: `git show 710b1ad:docs/fa/journal.md | grep -oE 'JR-I-[0-9]+'` → JR-I-1..13
    # (спека §0 стр. 12). Суффикс-НЕПУСТОЙ набор обязателен: одиночный `JR-I-1` из
    # первой редакции был суффикс-свободен, и сценарий B4DEAD зеленел СЛУЧАЙНОСТЬЮ
    # состава — фикс R-079 Б-1 (`\b` вместо `-F`) не пиннился ничем (`R-081` Б-1).
    # Здесь живой `JR-I-1` — ПРЕФИКС живых `JR-I-10..13` и мёртвого `JR-I-14`.
    printf '# journal FA\n\n' > docs/fa/journal.md
    for n in 1 2 3 4 5 6 7 8 9 10 11 12 13; do
      printf 'JR-I-%s — инвариант журнала\n' "${n}" >> docs/fa/journal.md
    done
    printf '\nDET-I-1\n' >> docs/fa/journal.md
    printf '# viz FA\n\nVB-I-1\nGS-I-1\n' > docs/fa/viz-backend.md
    printf '# venues FA\n\nVN-I-1\n' > docs/fa/venues.md
    printf '# reviews holder\n' > research/reviews/.keep
    printf 'pub fn journal_base() {}\n' > crates/journal/src/lib.rs
    printf 'pub fn gateway_base() {}\n' > crates/gateway/src/lib.rs
    printf 'pub fn venue_base() {}\n' > crates/venue-xyz/src/lib.rs
    printf 'pub fn recorder_base() {}\n' > crates/recorder/src/lib.rs
    printf 'base\n' > docs/readme.md
    git add -A
    git -c user.name=red-review-fa -c user.email=red-review-fa@noreply.local \
      commit -q -m "base"
  ) || die "${name}: базовая фикстура не создана"
  printf '%s\n' "${d}"
}

commit_all() {
  local repo="$1" subject="$2" body="${3:-}"
  (
    cd "${repo}" || exit 1
    git add -A
    if [ -n "${body}" ]; then
      git -c user.name=red-review-fa -c user.email=red-review-fa@noreply.local \
        commit -q -F - <<EOF_COMMIT
${subject}

${body}
EOF_COMMIT
    else
      git -c user.name=red-review-fa -c user.email=red-review-fa@noreply.local \
        commit -q -m "${subject}"
    fi
  ) || die "commit не состоялся: ${subject}"
}

base_sha() { ( cd "$1" && git rev-parse HEAD ); }

touch_crate() {
  local repo="$1" crate="$2" text="$3"
  mkdir -p "${repo}/crates/${crate}/src"
  printf '%s\n' "${text}" >> "${repo}/crates/${crate}/src/lib.rs"
}

add_review() {
  local repo="$1" file="$2" body="$3"
  mkdir -p "${repo}/research/reviews"
  printf '# %s\n\n%s\n' "$(basename "${file}")" "${body}" > "${repo}/${file}"
}

setup_assert() {
  local name="$1" repo="$2" why="$3" condition="$4"
  ( cd "${repo}" && eval "${condition}" ) >/dev/null 2>&1 \
    || die "${name}: SETUP не состоялся — ${why}"
}

range_guard_check() {
  local name="$1" repo="$2" base="$3"
  [ "${base}" = "${ZERO}" ] && return 0
  [ -z "${base}" ] && return 0
  ( cd "${repo}" && git rev-parse -q --verify "${base}^{commit}" >/dev/null 2>&1 ) || return 0
  if ( cd "${repo}" && git merge-base --is-ancestor "${base}" HEAD 2>/dev/null ); then
    local n
    n="$(cd "${repo}" && git rev-list --count "${base}..HEAD" 2>/dev/null)" || n=0
    if [ "${n:-0}" -lt 1 ]; then
      printf '%s: диапазон ПУСТ — фикстура не в задуманном состоянии, сценарий проверил бы пустоту вместо предмета' "${name}"
      return 1
    fi
  fi
  return 0
}

range_guard() {
  local msg
  msg="$(range_guard_check "$1" "$2" "$3")" || die "${msg}"
}

run_gate() {
  local name="$1" repo="$2" event="$3" base="$4"
  require_barrier "${name}"
  case "${event}" in
    push)
      LAST_OUT="$(cd "${repo}" && EVENT_NAME=push PUSH_BEFORE="${base}" PR_BASE_SHA="" bash "${BARRIER}" 2>&1)"
      LAST_RC=$?
      ;;
    pull_request)
      LAST_OUT="$(cd "${repo}" && EVENT_NAME=pull_request PUSH_BEFORE="" PR_BASE_SHA="${base}" bash "${BARRIER}" 2>&1)"
      LAST_RC=$?
      ;;
    empty_event)
      LAST_OUT="$(cd "${repo}" && EVENT_NAME="" PUSH_BEFORE="${base}" PR_BASE_SHA="" bash "${BARRIER}" 2>&1)"
      LAST_RC=$?
      ;;
    alien_event)
      LAST_OUT="$(cd "${repo}" && EVENT_NAME=schedule PUSH_BEFORE="${base}" PR_BASE_SHA="" bash "${BARRIER}" 2>&1)"
      LAST_RC=$?
      ;;
    *)
      die "${name}: неизвестная форма события в пробе: ${event}"
      ;;
  esac
}

expect_allow() {
  local name="$1" repo="$2" base="$3" desc="$4" event="${5:-push}" re="${6:-}"
  mark "${name}"
  range_guard "${name}" "${repo}" "${base}"
  run_gate "${name}" "${repo}" "${event}" "${base}"
  if [ "${LAST_RC}" -eq 0 ]; then
    if [ -n "${re}" ] && ! printf '%s\n' "${LAST_OUT}" | grep -qE "${re}"; then
      fail_scenario "${name} ${desc} — exit=0, но вывод не несёт ожидаемый маркер /${re}/"
    else
      pass_scenario "${name} ${desc} — пропущено"
    fi
  else
    fail_scenario "${name} ${desc} — ложное срабатывание (exit=${LAST_RC})"
    printf '%s\n' "${LAST_OUT}" | sed 's/^/      ↳ /' | head -8
  fi
}

expect_block() {
  local name="$1" repo="$2" base="$3" desc="$4" event="${5:-push}"
  mark "${name}"
  range_guard "${name}" "${repo}" "${base}"
  run_gate "${name}" "${repo}" "${event}" "${base}"
  if [ "${LAST_RC}" -ne 0 ]; then
    pass_scenario "${name} ${desc} — заблокировано"
  else
    fail_scenario "${name} ${desc} — ПРОШЛО"
    printf '%s\n' "${LAST_OUT}" | sed 's/^/      ↳ /' | head -8
  fi
}

expect_skip() {
  local name="$1" repo="$2" base="$3" desc="$4"
  mark "${name}"
  range_guard "${name}" "${repo}" "${base}"
  run_gate "${name}" "${repo}" push "${base}"
  if [ "${LAST_RC}" -eq 0 ] && printf '%s\n' "${LAST_OUT}" | grep -q 'SKIP'; then
    pass_scenario "${name} ${desc} — SKIP"
  elif [ "${LAST_RC}" -eq 0 ]; then
    fail_scenario "${name} ${desc} — exit=0, но нет явного SKIP"
  else
    fail_scenario "${name} ${desc} — должен был молчать, получил exit=${LAST_RC}"
    printf '%s\n' "${LAST_OUT}" | sed 's/^/      ↳ /' | head -8
  fi
}

fixture_live() {
  local name="$1" r b
  r="$(mk_repo "${name}")"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn ${name}_journal() {}"
  add_review "${r}" research/reviews/R-100-${name}.md "JR-I-1"
  commit_all "${r}" "${name}: journal with live FA echo"
  setup_assert "${name}" "${r}" "диф обязан трогать crates/journal и вводить R-файл" \
    "git diff --name-only '${b}' HEAD | grep -qx 'crates/journal/src/lib.rs' && git diff --name-only --diff-filter=A '${b}' HEAD | grep -qx 'research/reviews/R-100-${name}.md'"
  printf '%s|%s\n' "${r}" "${b}"
}

positive_control() {
  local pair r b
  pair="$(fixture_live positive)"; r="${pair%%|*}"; b="${pair##*|}"
  run_gate "POSITIVE-CONTROL" "${r}" push "${b}"
  if [ "${LAST_RC}" -ne 0 ]; then
    echo "SETUP НЕ СОСТОЯЛСЯ: POSITIVE-CONTROL: заведомо годная фикстура дала exit=${LAST_RC}; отказ барьера неотличим от честного срабатывания kill-сценариев." >&2
    printf '%s\n' "${LAST_OUT}" | sed 's/^/      ↳ /' >&2
    exit 2
  fi
  printf '%s\n' "${LAST_OUT}" | grep -qE 'JR-I-1' \
    || die "POSITIVE-CONTROL: барьер дал exit=0, но не предъявил живое эхо JR-I-1"
}

scenario_D1DOC() {
  local r b
  r="$(mk_repo d1doc)"; b="$(base_sha "${r}")"
  printf 'doc\n' >> "${r}/docs/readme.md"
  commit_all "${r}" "D1DOC: docs only"
  setup_assert D1DOC "${r}" "диф обязан быть doc-only, без crates/**" \
    "! git diff --name-only '${b}' HEAD | grep -q '^crates/'"
  expect_skip D1DOC "${r}" "${b}" "doc-only без crates/**"
}

scenario_D1NOREV() {
  local r b
  r="$(mk_repo d1norev)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn d1norev() {}"
  commit_all "${r}" "D1NOREV: crates without review"
  setup_assert D1NOREV "${r}" "диф обязан трогать crates/** и не вводить R-файл" \
    "git diff --name-only '${b}' HEAD | grep -q '^crates/' && ! git diff --name-only --diff-filter=A '${b}' HEAD | grep -q '^research/reviews/R-.*\\.md$'"
  expect_block D1NOREV "${r}" "${b}" "crates/** тронут, S=∅"
}

scenario_D1REVPAIR() {
  local r b
  r="$(mk_repo d1revpair)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn d1revpair_tmp() {}"
  commit_all "${r}" "D1REVPAIR: introduce crates change"
  git -C "${r}" checkout -q "${b}" -- crates/journal/src/lib.rs || die "D1REVPAIR: checkout base"
  commit_all "${r}" "D1REVPAIR: revert crates change"
  setup_assert D1REVPAIR "${r}" "net-diff BASE..HEAD обязан быть чист по crates/**" \
    "! git diff --name-only '${b}' HEAD | grep -q '^crates/' && [ \"\$(git rev-list --count '${b}'..HEAD)\" -eq 2 ]"
  expect_skip D1REVPAIR "${r}" "${b}" "реверт-пара, net-diff чист"
}

scenario_D1NOFA() {
  local r b
  r="$(mk_repo d1nofa)"; b="$(base_sha "${r}")"
  touch_crate "${r}" recorder "pub fn d1nofa() {}"
  add_review "${r}" research/reviews/R-101-d1nofa.md "review without waiver"
  commit_all "${r}" "D1NOFA: recorder without waiver"
  setup_assert D1NOFA "${r}" "должен быть только NO-FA recorder и R без waiver" \
    "git diff --name-only '${b}' HEAD | grep -q '^crates/recorder/' && ! grep -R 'FA-WAIVER:' research/reviews"
  expect_block D1NOFA "${r}" "${b}" "NO-FA recorder без waiver"
}

scenario_D1NOFAW() {
  local r b
  r="$(mk_repo d1nofaw)"; b="$(base_sha "${r}")"
  touch_crate "${r}" recorder "pub fn d1nofaw() {}"
  add_review "${r}" research/reviews/R-102-d1nofaw.md "FA-WAIVER: crates/recorder — 123456789012"
  commit_all "${r}" "D1NOFAW: recorder with waiver"
  setup_assert D1NOFAW "${r}" "waiver обязан назвать recorder и иметь причину ровно/более 12 символов" \
    "grep -q 'FA-WAIVER: crates/recorder — 123456789012' research/reviews/R-102-d1nofaw.md"
  expect_allow D1NOFAW "${r}" "${b}" "NO-FA recorder с waiver" push 'WAIVED'
}

scenario_D1NOFANOREV() {
  local r b
  r="$(mk_repo d1nofanorev)"; b="$(base_sha "${r}")"
  touch_crate "${r}" recorder "pub fn d1nofanorev() {}"
  commit_all "${r}" "D1NOFANOREV: recorder without review"
  setup_assert D1NOFANOREV "${r}" "recorder изменён, S=∅" \
    "git diff --name-only '${b}' HEAD | grep -q '^crates/recorder/' && ! git diff --name-only --diff-filter=A '${b}' HEAD | grep -q '^research/reviews/R-.*\\.md$'"
  expect_block D1NOFANOREV "${r}" "${b}" "NO-FA recorder без review"
}

scenario_M2KNOWN() {
  local pair r b
  pair="$(fixture_live m2known)"; r="${pair%%|*}"; b="${pair##*|}"
  expect_allow M2KNOWN "${r}" "${b}" "известный FA-крейт journal принимается" push 'JR-I-1'
}

scenario_M2UNKNOWN() {
  local r b
  r="$(mk_repo m2unknown)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal2 "pub fn m2unknown() {}"
  add_review "${r}" research/reviews/R-103-m2unknown.md "JR-I-1"
  commit_all "${r}" "M2UNKNOWN: unknown crate"
  setup_assert M2UNKNOWN "${r}" "диф обязан трогать незнакомый crates/journal2" \
    "git diff --name-only '${b}' HEAD | grep -q '^crates/journal2/'"
  expect_block M2UNKNOWN "${r}" "${b}" "незнакомое имя крейта"
}

scenario_M2VENUE() {
  local r b
  r="$(mk_repo m2venue)"; b="$(base_sha "${r}")"
  touch_crate "${r}" venue-xyz "pub fn m2venue() {}"
  add_review "${r}" research/reviews/R-104-m2venue.md "VN-I-1"
  commit_all "${r}" "M2VENUE: venue glob"
  setup_assert M2VENUE "${r}" "venue-* обязан идти через docs/fa/venues.md" \
    "git diff --name-only '${b}' HEAD | grep -q '^crates/venue-xyz/' && grep -q 'VN-I-1' docs/fa/venues.md"
  expect_allow M2VENUE "${r}" "${b}" "venue-* glob с живым VN-I-*" push 'VN-I-1'
}

scenario_S3ADDED() {
  local pair r b
  pair="$(fixture_live s3added)"; r="${pair%%|*}"; b="${pair##*|}"
  expect_allow S3ADDED "${r}" "${b}" "R введён диапазоном" push 'JR-I-1'
}

scenario_S3NAMED() {
  local r b
  r="$(mk_repo s3named)"
  add_review "${r}" research/reviews/R-105-existing.md "JR-I-1"
  commit_all "${r}" "S3NAMED: existing review before range"
  b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn s3named() {}"
  commit_all "${r}" "S3NAMED: code names review" "research/reviews/R-105-existing.md"
  setup_assert S3NAMED "${r}" "R-файл обязан существовать на HEAD, но не быть добавленным диапазоном" \
    "test -f research/reviews/R-105-existing.md && ! git diff --name-only --diff-filter=A '${b}' HEAD | grep -q '^research/reviews/R-105-existing.md$'"
  expect_allow S3NAMED "${r}" "${b}" "R назван полным путём в commit message" push 'JR-I-1'
}

scenario_S3GHOST() {
  local r b
  r="$(mk_repo s3ghost)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn s3ghost() {}"
  commit_all "${r}" "S3GHOST: code names ghost" "research/reviews/R-999-ghost.md"
  setup_assert S3GHOST "${r}" "названный R-путь обязан отсутствовать на HEAD" \
    "! test -e research/reviews/R-999-ghost.md"
  expect_block S3GHOST "${r}" "${b}" "ghost review не входит в S"
}

scenario_S3BAREID() {
  local r b
  r="$(mk_repo s3bareid)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn s3bareid() {}"
  commit_all "${r}" "S3BAREID: R-106 mentioned without path"
  setup_assert S3BAREID "${r}" "в commit message нет полного пути research/reviews/R-*.md" \
    "! git log -1 --format=%B | grep -q 'research/reviews/R-'"
  expect_block S3BAREID "${r}" "${b}" "голый R-NNN не входит в S"
}

scenario_B4LIVE() {
  local pair r b
  pair="$(fixture_live b4live)"; r="${pair%%|*}"; b="${pair##*|}"
  expect_allow B4LIVE "${r}" "${b}" "живой ID своего префикса" push 'JR-I-1'
}

scenario_B4CROSSCUT() {
  local r b
  r="$(mk_repo b4crosscut)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn b4crosscut() {}"
  add_review "${r}" research/reviews/R-107-b4crosscut.md "DET-I-1"
  commit_all "${r}" "B4CROSSCUT: cross-cutting id only"
  setup_assert B4CROSSCUT "${r}" "FA journal цитирует DET-I-1, но review не несёт JR-I-*" \
    "grep -q 'DET-I-1' docs/fa/journal.md && ! grep -q 'JR-I-' research/reviews/R-107-b4crosscut.md"
  expect_block B4CROSSCUT "${r}" "${b}" "сквозной DET-I-1 не доказывает открытие FA journal"
}

scenario_B4FOREIGN() {
  local r b
  r="$(mk_repo b4foreign)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn b4foreign() {}"
  add_review "${r}" research/reviews/R-108-b4foreign.md "VB-I-1"
  commit_all "${r}" "B4FOREIGN: foreign id"
  setup_assert B4FOREIGN "${r}" "диф только journal, ID только VB" \
    "git diff --name-only '${b}' HEAD | grep -q '^crates/journal/' && ! git diff --name-only '${b}' HEAD | grep -q '^crates/gateway/'"
  expect_block B4FOREIGN "${r}" "${b}" "ID чужой FA"
}

scenario_B4DEAD() {
  local r b
  r="$(mk_repo b4dead)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn b4dead() {}"
  add_review "${r}" research/reviews/R-109-b4dead.md "JR-I-999"
  commit_all "${r}" "B4DEAD: dead id"
  setup_assert B4DEAD "${r}" "JR-I-999 обязан отсутствовать в docs/fa/journal.md" \
    "! grep -q 'JR-I-999' docs/fa/journal.md"
  expect_block B4DEAD "${r}" "${b}" "синтаксически валидный, но мёртвый ID"
}

scenario_B4UNION() {
  local r b
  r="$(mk_repo b4union)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn b4union_journal() {}"
  touch_crate "${r}" gateway "pub fn b4union_gateway() {}"
  add_review "${r}" research/reviews/R-110-b4union.md "JR-I-1"
  commit_all "${r}" "B4UNION: journal and gateway, one live id"
  setup_assert B4UNION "${r}" "диф обязан трогать два FA-крейта, review несёт только JR-I-1" \
    "git diff --name-only '${b}' HEAD | grep -q '^crates/journal/' && git diff --name-only '${b}' HEAD | grep -q '^crates/gateway/' && ! grep -q 'VB-I-' research/reviews/R-110-b4union.md"
  expect_allow B4UNION "${r}" "${b}" "union-B: живой ID одного из тронутых крейтов достаточен" push 'JR-I-1'
}

scenario_B4DEADPFX() {
  local r b
  r="$(mk_repo b4deadpfx)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn b4deadpfx() {}"
  add_review "${r}" research/reviews/R-120-b4deadpfx.md "JR-I-14"
  commit_all "${r}" "B4DEADPFX: dead id whose prefix is a live id"
  # Setup-guard давит на САМ инвариант: JR-I-14 обязан быть мёртвым, а живой JR-I-1
  # обязан быть его ПОДСТРОКОЙ — иначе сценарий проверяет не тот класс (`testing.md`
  # §«Целостность гейта» св-во 3: проба, молча тестирующая не тот сценарий, — плацебо).
  setup_assert B4DEADPFX "${r}" "JR-I-14 мёртв, JR-I-1 жив и является его префиксом" \
    "! grep -qE '\\bJR-I-14\\b' docs/fa/journal.md && grep -qE '\\bJR-I-1\\b' docs/fa/journal.md && case JR-I-14 in JR-I-1*) true ;; *) false ;; esac"
  expect_block B4DEADPFX "${r}" "${b}" "мёртвый ID, содержащий живой как префикс"
}

scenario_B4LIVETAIL() {
  local r b
  r="$(mk_repo b4livetail)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn b4livetail() {}"
  add_review "${r}" research/reviews/R-121-b4livetail.md "JR-I-13"
  commit_all "${r}" "B4LIVETAIL: live tail id only"
  setup_assert B4LIVETAIL "${r}" "JR-I-13 жив, в вердикте НЕТ голого JR-I-1" \
    "grep -qE '\\bJR-I-13\\b' docs/fa/journal.md && ! grep -qE '\\bJR-I-1\\b' research/reviews/R-121-b4livetail.md"
  expect_allow B4LIVETAIL "${r}" "${b}" "живой ID-хвост набора" push 'JR-I-13'
}

scenario_B4EMPTYU() {
  local r b
  r="$(mk_repo b4emptyu)"; b="$(base_sha "${r}")"
  printf '# journal FA\n\nDET-I-1\n' > "${r}/docs/fa/journal.md"
  touch_crate "${r}" journal "pub fn b4emptyu() {}"
  add_review "${r}" research/reviews/R-122-b4emptyu.md "JR-I-1"
  commit_all "${r}" "B4EMPTYU: FA file present but carries no own-prefix id"
  setup_assert B4EMPTYU "${r}" "docs/fa/journal.md существует и НЕ несёт ни одного JR-I-*" \
    "test -f docs/fa/journal.md && ! grep -qE '\\bJR-I-[0-9]+\\b' docs/fa/journal.md && git diff --name-only '${b}' HEAD | grep -q '^crates/journal/'"
  expect_block B4EMPTYU "${r}" "${b}" "U=∅ при непустом LIVE_CRA — тихий отказ гейта"
}

scenario_E5EVENT() {
  local pair r b ok=1
  mark E5EVENT
  pair="$(fixture_live e5event)"; r="${pair%%|*}"; b="${pair##*|}"
  range_guard E5EVENT "${r}" "${b}"
  run_gate E5EMPTY "${r}" empty_event "${b}"
  [ "${LAST_RC}" -ne 0 ] || { echo "FAIL  E5EMPTY пустой EVENT_NAME прошёл"; ok=0; }
  run_gate E5ALIEN "${r}" alien_event "${b}"
  [ "${LAST_RC}" -ne 0 ] || { echo "FAIL  E5ALIEN неизвестный EVENT_NAME прошёл"; ok=0; }
  if [ "${ok}" -eq 1 ]; then pass_scenario "E5EMPTY · E5ALIEN — оба fail-closed"
  else fail_scenario "E5EVENT — не все формы EVENT_NAME fail-closed"; fi
}

scenario_E5BASE() {
  local pair r b side lost ok=1
  mark E5BASE
  pair="$(fixture_live e5base)"; r="${pair%%|*}"; b="${pair##*|}"
  range_guard E5BASE "${r}" "${b}"
  run_gate E5BASE0 "${r}" push ""
  [ "${LAST_RC}" -ne 0 ] || { echo "FAIL  E5BASE0 пустая база прошла"; ok=0; }
  run_gate E5ZERO "${r}" push "${ZERO}"
  [ "${LAST_RC}" -ne 0 ] || { echo "FAIL  E5ZERO zero-SHA прошёл"; ok=0; }
  lost=1111111111111111111111111111111111111111
  run_gate E5LOST "${r}" push "${lost}"
  [ "${LAST_RC}" -ne 0 ] || { echo "FAIL  E5LOST отсутствующая база прошла"; ok=0; }

  r="$(mk_repo e5nonanc)"
  git -C "${r}" checkout -q -b side
  printf 'side\n' >> "${r}/docs/readme.md"
  commit_all "${r}" "E5NONANC: side base"
  side="$(git -C "${r}" rev-parse HEAD)"
  git -C "${r}" checkout -q main
  b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn e5nonanc() {}"
  add_review "${r}" research/reviews/R-111-e5nonanc.md "JR-I-1"
  commit_all "${r}" "E5NONANC: main code"
  setup_assert E5NONANC "${r}" "side SHA обязан существовать и НЕ быть предком HEAD" \
    "git cat-file -e '${side}^{commit}' && ! git merge-base --is-ancestor '${side}' HEAD"
  run_gate E5NONANC "${r}" push "${side}"
  [ "${LAST_RC}" -ne 0 ] || { echo "FAIL  E5NONANC база не-предок прошла"; ok=0; }

  if [ "${ok}" -eq 1 ]; then pass_scenario "E5BASE0 · E5ZERO · E5LOST · E5NONANC — все fail-closed"
  else fail_scenario "E5BASE — не все недостоверные базы fail-closed"; fi
}

scenario_E5OK() {
  local pair r b ok=1
  mark E5OK
  pair="$(fixture_live e5ok)"; r="${pair%%|*}"; b="${pair##*|}"
  range_guard E5OK "${r}" "${b}"
  run_gate E5PUSH "${r}" push "${b}"
  [ "${LAST_RC}" -eq 0 ] || { echo "FAIL  E5PUSH легитимный push дал exit=${LAST_RC}"; ok=0; }
  run_gate E5PR "${r}" pull_request "${b}"
  [ "${LAST_RC}" -eq 0 ] || { echo "FAIL  E5PR легитимный PR дал exit=${LAST_RC}"; ok=0; }
  if [ "${ok}" -eq 1 ]; then pass_scenario "E5PUSH · E5PR — обе формы события проходят"
  else fail_scenario "E5OK — легитимная форма события дала ложное красное"; fi
}

scenario_F6NOFAFILE() {
  local r b
  r="$(mk_repo f6nofafile)"; b="$(base_sha "${r}")"
  rm -f "${r}/docs/fa/journal.md"
  touch_crate "${r}" journal "pub fn f6nofafile() {}"
  add_review "${r}" research/reviews/R-112-f6nofafile.md "JR-I-1"
  commit_all "${r}" "F6NOFAFILE: mapped FA file absent"
  setup_assert F6NOFAFILE "${r}" "docs/fa/journal.md обязан отсутствовать на HEAD" \
    "! test -f docs/fa/journal.md"
  expect_block F6NOFAFILE "${r}" "${b}" "FA-файл маппинга отсутствует"
}

scenario_F6NODIR() {
  local r b
  r="$(mk_repo f6nodir)"; b="$(base_sha "${r}")"
  rm -rf "${r}/docs/fa"
  touch_crate "${r}" journal "pub fn f6nodir() {}"
  add_review "${r}" research/reviews/R-113-f6nodir.md "JR-I-1"
  commit_all "${r}" "F6NODIR: docs/fa absent"
  setup_assert F6NODIR "${r}" "каталог docs/fa обязан отсутствовать на HEAD" \
    "! test -d docs/fa"
  expect_block F6NODIR "${r}" "${b}" "каталога docs/fa/ нет"
}

scenario_F6NOREV() {
  local r b
  r="$(mk_repo f6norev)"; b="$(base_sha "${r}")"
  rm -rf "${r}/research/reviews"
  touch_crate "${r}" journal "pub fn f6norev() {}"
  commit_all "${r}" "F6NOREV: no reviews dir"
  setup_assert F6NOREV "${r}" "каталог research/reviews обязан отсутствовать на HEAD" \
    "! test -d research/reviews && git diff --name-only '${b}' HEAD | grep -q '^crates/journal/'"
  expect_block F6NOREV "${r}" "${b}" "research/reviews/ отсутствует"
}

scenario_F6SETUPGUARD() {
  local r b msg rc
  mark F6SETUPGUARD
  r="$(mk_repo f6setupguard)"; b="$(base_sha "${r}")"
  msg="$(range_guard_check F6SETUPGUARD "${r}" "${b}")"; rc=$?
  if [ "${rc}" -ne 0 ] && printf '%s\n' "${msg}" | grep -q 'диапазон ПУСТ'; then
    pass_scenario "F6SETUPGUARD сломанная фикстура поймана ДО барьера: ${msg}"
  else
    fail_scenario "F6SETUPGUARD не поймал пустой диапазон (rc=${rc}, msg=${msg})"
  fi
}

scenario_W7WRONG() {
  local r b
  r="$(mk_repo w7wrong)"; b="$(base_sha "${r}")"
  touch_crate "${r}" recorder "pub fn w7wrong() {}"
  add_review "${r}" research/reviews/R-114-w7wrong.md "FA-WAIVER: crates/derive — 123456789012"
  commit_all "${r}" "W7WRONG: waiver names wrong crate"
  setup_assert W7WRONG "${r}" "waiver обязан называть derive при тронутом recorder" \
    "grep -q 'FA-WAIVER: crates/derive' research/reviews/R-114-w7wrong.md && ! grep -q 'FA-WAIVER: crates/recorder' research/reviews/R-114-w7wrong.md"
  expect_block W7WRONG "${r}" "${b}" "waiver не токен на предъявителя"
}

scenario_W7MIXGAP() {
  local r b
  r="$(mk_repo w7mixgap)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn w7mixgap_journal() {}"
  touch_crate "${r}" recorder "pub fn w7mixgap_recorder() {}"
  add_review "${r}" research/reviews/R-115-w7mixgap.md "JR-I-1"
  commit_all "${r}" "W7MIXGAP: echo but no waiver"
  setup_assert W7MIXGAP "${r}" "диф обязан смешивать journal+recorder без waiver" \
    "git diff --name-only '${b}' HEAD | grep -q '^crates/journal/' && git diff --name-only '${b}' HEAD | grep -q '^crates/recorder/' && ! grep -R 'FA-WAIVER:' research/reviews"
  expect_block W7MIXGAP "${r}" "${b}" "живое эхо FA-крейта не гасит NO-FA пробел"
}

scenario_W7MIXW() {
  local r b
  r="$(mk_repo w7mixw)"; b="$(base_sha "${r}")"
  touch_crate "${r}" journal "pub fn w7mixw_journal() {}"
  touch_crate "${r}" recorder "pub fn w7mixw_recorder() {}"
  add_review "${r}" research/reviews/R-116-w7mixw.md "JR-I-1
FA-WAIVER: crates/recorder — 123456789012"
  commit_all "${r}" "W7MIXW: echo and waiver"
  setup_assert W7MIXW "${r}" "review обязан нести и JR-I-1, и waiver recorder" \
    "grep -q 'JR-I-1' research/reviews/R-116-w7mixw.md && grep -q 'FA-WAIVER: crates/recorder — 123456789012' research/reviews/R-116-w7mixw.md"
  expect_allow W7MIXW "${r}" "${b}" "mixed diff с эхом и waiver" push 'WAIVED|JR-I-1'
}

scenario_W7EMPTY() {
  local r b
  r="$(mk_repo w7empty)"; b="$(base_sha "${r}")"
  touch_crate "${r}" recorder "pub fn w7empty() {}"
  add_review "${r}" research/reviews/R-117-w7empty.md "FA-WAIVER: crates/recorder — "
  commit_all "${r}" "W7EMPTY: empty waiver reason"
  setup_assert W7EMPTY "${r}" "строка waiver обязана быть пустой после тире" \
    "grep -qx 'FA-WAIVER: crates/recorder — ' research/reviews/R-117-w7empty.md"
  expect_block W7EMPTY "${r}" "${b}" "пустая причина waiver"
}

scenario_W7SHORT() {
  local r b
  r="$(mk_repo w7short)"; b="$(base_sha "${r}")"
  touch_crate "${r}" recorder "pub fn w7short() {}"
  add_review "${r}" research/reviews/R-118-w7short.md "FA-WAIVER: crates/recorder — 12345678901"
  commit_all "${r}" "W7SHORT: short waiver reason"
  setup_assert W7SHORT "${r}" "причина должна быть 11 символов" \
    "grep -qx 'FA-WAIVER: crates/recorder — 12345678901' research/reviews/R-118-w7short.md"
  expect_block W7SHORT "${r}" "${b}" "причина на один символ короче порога"
}

scenario_W7EXACT() {
  local r b
  r="$(mk_repo w7exact)"; b="$(base_sha "${r}")"
  touch_crate "${r}" recorder "pub fn w7exact() {}"
  add_review "${r}" research/reviews/R-119-w7exact.md "FA-WAIVER: crates/recorder — 123456789012"
  commit_all "${r}" "W7EXACT: exact waiver reason"
  setup_assert W7EXACT "${r}" "причина должна быть ровно 12 символов" \
    "grep -qx 'FA-WAIVER: crates/recorder — 123456789012' research/reviews/R-119-w7exact.md"
  expect_allow W7EXACT "${r}" "${b}" "причина ровно в порог" push 'WAIVED'
}

scenario_W7PFX() {
  local r b
  r="$(mk_repo w7pfx)"; b="$(base_sha "${r}")"
  touch_crate "${r}" recorder "pub fn w7pfx() {}"
  add_review "${r}" research/reviews/R-123-w7pfx.md "FA-WAIVER: crates/recorder2 — 123456789012"
  commit_all "${r}" "W7PFX: waiver names a crate whose name extends the touched one"
  setup_assert W7PFX "${r}" "waiver обязан называть recorder2 и НЕ называть recorder как отдельное слово" \
    "grep -q 'FA-WAIVER: crates/recorder2 — ' research/reviews/R-123-w7pfx.md && ! grep -qE '^FA-WAIVER: crates/recorder — ' research/reviews/R-123-w7pfx.md"
  expect_block W7PFX "${r}" "${b}" "waiver называет крейт-суффикс тронутого"
}

write_workflow() {
  local file="$1" mode="$2"
  mkdir -p "$(dirname "${file}")"
  case "${mode}" in
    good)
      cat > "${file}" <<'EOF_YAML'
name: fx
on: [push]
jobs:
  build-test:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test --all
  review-fa:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - env:
          EVENT_NAME: ${{ github.event_name }}
          PUSH_BEFORE: ${{ github.event.before }}
          PR_BASE_SHA: ${{ github.event.pull_request.base.sha }}
        run: bash scripts/check_review_fa.sh
      - run: bash scripts/tests/red_review_fa.sh
  status-check:
    runs-on: ubuntu-latest
    needs: [build-test, review-fa]
    if: always()
    steps:
      - run: |
          if [[ "${{ needs.build-test.result }}" != "success" || "${{ needs.review-fa.result }}" != "success" ]]; then
            echo fail; exit 1
          fi
          echo ok
EOF_YAML
      ;;
    noneeds)
      write_workflow "${file}" good
      python3 - "${file}" <<'PY'
from pathlib import Path
p = Path(__import__("sys").argv[1])
s = p.read_text()
s = s.replace("needs: [build-test, review-fa]", "needs: [build-test]")
p.write_text(s)
PY
      ;;
    noif)
      write_workflow "${file}" good
      python3 - "${file}" <<'PY'
from pathlib import Path
p = Path(__import__("sys").argv[1])
s = p.read_text()
s = s.replace(' || "${{ needs.review-fa.result }}" != "success"', '')
p.write_text(s)
PY
      ;;
    cmtonly)
      # Мутант D вердикта `R-081` §5 N-5: исполняемые строки `with:`/`fetch-depth: 0`
      # СНЯТЫ, а комментарий с тем же текстом оставлен. Джоб реально идёт с depth=1;
      # оракул, ищущий иглу по всему тексту блока, зеленеет на комментарии.
      write_workflow "${file}" good
      python3 - "${file}" <<'PY'
from pathlib import Path
p = Path(__import__("sys").argv[1])
s = p.read_text()
s = s.replace(
    "      - uses: actions/checkout@v4\n        with:\n          fetch-depth: 0\n",
    "      # fetch-depth: 0 ОБЯЗАТЕЛЬНО: барьер строит диапазон $BASE..HEAD\n"
    "      - uses: actions/checkout@v4\n",
)
p.write_text(s)
PY
      ;;
    nocall)
      write_workflow "${file}" good
      python3 - "${file}" <<'PY'
from pathlib import Path
p = Path(__import__("sys").argv[1])
s = p.read_text()
s = s.replace("run: bash scripts/check_review_fa.sh", "run: echo no barrier")
s = s.replace("run: bash scripts/tests/red_review_fa.sh", "run: echo no probe")
p.write_text(s)
PY
      ;;
    *) die "unknown workflow mode ${mode}" ;;
  esac
}

check_wiring_file() {
  python3 - "$1" <<'PY'
import re, sys
from pathlib import Path

path = Path(sys.argv[1])
if not path.exists():
    print(f"workflow missing: {path}")
    sys.exit(1)
# Иглы ищутся по ИСПОЛНЯЕМЫМ строкам: строка-комментарий YAML выбрасывается целиком.
# Без этого `fetch-depth: 0` находится в комментарии джоба, и снятие самой директивы
# даёт ложный зелёный (`R-081` §5 N-5, мутант D).
text = "\n".join(
    line for line in path.read_text(encoding="utf-8").splitlines()
    if not line.lstrip().startswith("#")
)

def job_block(name: str) -> str:
    m = re.search(rf"(?ms)^  {re.escape(name)}:\n(.*?)(?=^  [A-Za-z0-9_-]+:\n|\Z)", text)
    return m.group(1) if m else ""

review = job_block("review-fa")
status = job_block("status-check")
ok = True
if not review:
    print("review-fa job missing")
    ok = False
if not status:
    print("status-check job missing")
    ok = False

for needle in ("fetch-depth: 0", "EVENT_NAME:", "PUSH_BEFORE:", "PR_BASE_SHA:",
               "bash scripts/check_review_fa.sh", "bash scripts/tests/red_review_fa.sh"):
    if needle not in review:
        print(f"review-fa missing {needle}")
        ok = False

needs_line = next((line for line in status.splitlines() if line.strip().startswith("needs:")), "")
if "review-fa" not in needs_line:
    print("status-check.needs missing review-fa")
    ok = False
if "needs.review-fa.result" not in status:
    print("status-check if/run condition missing needs.review-fa.result")
    ok = False

sys.exit(0 if ok else 1)
PY
}

expect_wiring() {
  local name="$1" file="$2" expected="$3" desc="$4" out rc
  mark "${name}"
  out="$(check_wiring_file "${file}" 2>&1)"; rc=$?
  if [ "${expected}" = pass ] && [ "${rc}" -eq 0 ]; then
    pass_scenario "${name} ${desc} — wiring OK"
  elif [ "${expected}" = fail ] && [ "${rc}" -ne 0 ]; then
    pass_scenario "${name} ${desc} — wiring FAIL как ожидалось"
  elif [ "${expected}" = pass ]; then
    fail_scenario "${name} ${desc} — wiring красный"
    printf '%s\n' "${out}" | sed 's/^/      ↳ /' | head -8
  else
    fail_scenario "${name} ${desc} — wiring прошёл, хотя обязан краснеть"
  fi
}

scenario_W8NONEEDS() {
  local d f
  d="$(mktemp -d /tmp/red-review-fa-w8noneeds-XXXXXX)" || die W8NONEEDS
  register_dir "${d}"; f="${d}/ci.yml"; write_workflow "${f}" noneeds
  expect_wiring W8NONEEDS "${f}" fail "review-fa не входит в status-check.needs"
}

scenario_W8NOIF() {
  local d f
  d="$(mktemp -d /tmp/red-review-fa-w8noif-XXXXXX)" || die W8NOIF
  register_dir "${d}"; f="${d}/ci.yml"; write_workflow "${f}" noif
  expect_wiring W8NOIF "${f}" fail "review-fa есть в needs, но нет в условии агрегата"
}

scenario_W8NOCALL() {
  local d f
  d="$(mktemp -d /tmp/red-review-fa-w8nocall-XXXXXX)" || die W8NOCALL
  register_dir "${d}"; f="${d}/ci.yml"; write_workflow "${f}" nocall
  expect_wiring W8NOCALL "${f}" fail "review-fa не зовёт барьер или пробу"
}

scenario_W8OK() {
  expect_wiring W8OK "${CI}" pass "реальный ci.yml чекаута несёт полную проводку"
}

scenario_W8CMTONLY() {
  local d f
  d="$(mktemp -d /tmp/red-review-fa-w8cmtonly-XXXXXX)" || die W8CMTONLY
  register_dir "${d}"; f="${d}/ci.yml"; write_workflow "${f}" cmtonly
  setup_assert W8CMTONLY "${d}" "директива снята, а комментарий с иглой оставлен" \
    "grep -qE '^ *# .*fetch-depth: 0' ci.yml && ! grep -qE '^ *fetch-depth: 0 *$' ci.yml"
  expect_wiring W8CMTONLY "${f}" fail "fetch-depth: 0 остался только в комментарии"
}

emit_ref_barrier() {
  local file="$1"
  cat > "${file}" <<'EOF_REF'
#!/usr/bin/env bash
set -uo pipefail
ZERO=0000000000000000000000000000000000000000
M="${REVIEW_FA_MUTANT:-ref}"

raw="${1:-}"
if [ -z "${raw}" ]; then
  case "${EVENT_NAME:-}" in
    push) raw="${PUSH_BEFORE:-}" ;;
    pull_request) raw="${PR_BASE_SHA:-}" ;;
    "") exit 1 ;;
    *) exit 1 ;;
  esac
fi

if [ "${M}" != nobase ]; then
  [ -n "${raw}" ] || exit 1
  case "${raw}" in *[!0]*) : ;; *) exit 1 ;; esac
  git rev-parse -q --verify "${raw}^{commit}" >/dev/null 2>&1 || exit 1
  git merge-base --is-ancestor "${raw}" HEAD 2>/dev/null || exit 1
else
  [ -n "${raw}" ] || raw=HEAD
fi
BASE="$(git rev-parse -q --verify "${raw}^{commit}" 2>/dev/null || printf '%s' "${raw}")"

crates="$(git diff --name-only "${BASE}" HEAD 2>/dev/null | awk -F/ '$1=="crates" && $2!="" {print $2}' | sort -u)"
[ -n "${crates}" ] || { echo "SKIP (диапазон не трогает crates/**)"; exit 0; }

tmp="$(mktemp -d /tmp/ref-review-fa-XXXXXX)" || exit 1
trap 'rm -rf "${tmp}"' EXIT
ids="${tmp}/ids"; nofa="${tmp}/nofa"; live="${tmp}/live"
: >"${ids}"; : >"${nofa}"; : >"${live}"

for c in ${crates}; do
  fa=""; pfx=""
  case "${c}" in
    journal) fa=docs/fa/journal.md; pfx=JR ;;
    gateway) fa=docs/fa/viz-backend.md; pfx=VB ;;
    gateway-serve) fa=docs/fa/viz-backend.md; pfx=GS ;;
    venue-*) fa=docs/fa/venues.md; pfx=VN ;;
    recorder|derive) printf '%s\n' "${c}" >> "${nofa}"; continue ;;
    *)
      [ "${M}" = nomap ] && continue
      echo "FAIL unknown crate ${c}" >&2
      exit 1
      ;;
  esac
  if [ ! -f "${fa}" ]; then
    if [ "${M}" = emptyfaok ]; then
      printf '%s-I-1\n' "${pfx}" >> "${ids}"
      continue
    fi
    echo "FAIL missing FA ${fa}" >&2
    exit 1
  fi
  printf '%s\n' "${c}" >> "${live}"
  if [ "${M}" = anymodule ]; then
    grep -RhoE '\b[A-Z]{2,4}-I-[0-9]+\b' docs/fa/*.md 2>/dev/null >> "${ids}" || true
  elif [ "${M}" = anyid ]; then
    grep -hoE '\b[A-Z]{2,4}-I-[0-9]+\b' "${fa}" >> "${ids}" || true
  elif [ "${M}" = firstidonly ]; then
    grep -hoE "\b${pfx}-I-[0-9]+\b" "${fa}" 2>/dev/null | head -1 >> "${ids}" || true
  else
    grep -hoE "\b${pfx}-I-[0-9]+\b" "${fa}" >> "${ids}" || true
  fi
done
sort -u -o "${ids}" "${ids}"
if [ "${M}" = nomap ] && [ ! -s "${ids}" ] && [ ! -s "${nofa}" ]; then
  echo "SKIP mutant nomap ignored unknown crates"
  exit 0
fi

reviews="${tmp}/reviews"; : >"${reviews}"
git diff --name-status --diff-filter=A "${BASE}" HEAD -- 'research/reviews/*.md' 2>/dev/null \
  | awk '$1=="A"{print $2}' >> "${reviews}" || true
git log --format=%B "${BASE}..HEAD" 2>/dev/null \
  | grep -oE 'research/reviews/R-[^[:space:])]+\.md' >> "${reviews}" || true
sort -u -o "${reviews}" "${reviews}"

if [ ! -s "${reviews}" ]; then
  [ "${M}" = nod ] && { echo "mutant nod accepts S empty"; exit 0; }
  echo "FAIL no review files" >&2
  exit 1
fi

waiver_ok=1
while IFS= read -r c; do
  [ -z "${c}" ] && continue
  hit=1
  while IFS= read -r f; do
    if [ -f "${f}" ]; then
      if [ "${M}" = anywaiver ]; then
        grep -qE '^FA-WAIVER: crates/[^ ]+ — .{12,}$' "${f}" && hit=0
      elif [ "${M}" = substrwaiver ]; then
        grep -qF "FA-WAIVER: crates/${c}" "${f}" && hit=0
      else
        grep -qE "^FA-WAIVER: crates/${c} — .{12,}$" "${f}" && hit=0
      fi
    fi
  done < "${reviews}"
  if [ "${hit}" -ne 0 ]; then
    [ "${M}" = vacuousok ] && continue
    waiver_ok=0
    echo "FAIL missing waiver for crates/${c}" >&2
  fi
done < "${nofa}"

echo_ok=1
echo_seen=1
if [ -s "${ids}" ]; then
  echo_ok=0
  while IFS= read -r id; do
    [ -z "${id}" ] && continue
    while IFS= read -r f; do
      if [ -f "${f}" ]; then
        if [ "${M}" = substr ]; then
          grep -qF "${id}" "${f}" && { echo "${f}: ${id}"; echo_seen=0; }
        elif grep -qE "\b${id}\b" "${f}"; then
          echo "${f}: ${id}"
          echo_seen=0
        fi
      elif [ "${M}" = ghostok ]; then
        echo "${f}: ${id} (ghost mutant)"
        echo_seen=0
      fi
    done < "${reviews}"
  done < "${ids}"
  if [ "${M}" = synonly ] && [ "${echo_seen}" -ne 0 ]; then
    while IFS= read -r f; do
      [ -f "${f}" ] && grep -qE '\b[A-Z]{2,4}-I-[0-9]+\b' "${f}" && echo_seen=0
    done < "${reviews}"
  fi
else
  if [ -s "${live}" ] && [ "${M}" != vacuousecho ]; then
    echo "FAIL U = empty with non-empty live crates" >&2
    exit 1
  fi
  echo_ok=0
fi
if [ -s "${ids}" ] && [ "${echo_seen}" -ne 0 ]; then
  echo_ok=1
fi

if [ "${M}" = echoexcuse ] && [ "${echo_seen}" -eq 0 ]; then
  waiver_ok=1
fi

[ "${waiver_ok}" -eq 1 ] || exit 1
[ "${echo_ok}" -eq 0 ] || { echo "FAIL no live FA echo" >&2; exit 1; }

while IFS= read -r c; do [ -n "${c}" ] && echo "WAIVED: crates/${c}"; done < "${nofa}"
exit 0
EOF_REF
  chmod +x "${file}"
  bash -n "${file}" || die "эталонный барьер батареи не парсится"
}

run_battery() {
  local d ref wf rc bad=0 total=0 entry name scen
  d="$(mktemp -d /tmp/red-review-fa-battery-XXXXXX)" || die "battery mktemp"
  register_dir "${d}"
  ref="${d}/ref-check-review-fa.sh"
  wf="${d}/ci-good.yml"
  emit_ref_barrier "${ref}"
  write_workflow "${wf}" good

  echo "══ БАТАРЕЯ M-66: эталон зелёный, мутанты красные по §4 ══"
  env -u FIXTURES_REG BARRIER="${ref}" CI="${wf}" bash "${SELF}" > "${d}/ref.log" 2>&1; rc=$?
  total=$((total + 1))
  if [ "${rc}" -eq 0 ]; then
    echo "PASS  ref → $(grep -oE 'VERDICT: PASS \([0-9]+/[0-9]+\)' "${d}/ref.log" | head -1)"
  else
    echo "FAIL  ref → exit=${rc}"
    grep -E '^(FAIL|SETUP)' "${d}/ref.log" | head -10 | sed 's/^/      ↳ /'
    bad=$((bad + 1))
  fi

  BATTERY_ENTRIES="anyid:B4CROSSCUT
synonly:B4DEAD
substr:B4DEADPFX
firstidonly:B4LIVETAIL
vacuousecho:B4EMPTYU
anymodule:B4FOREIGN
nomap:M2UNKNOWN
nod:D1NOREV
ghostok:S3GHOST
nobase:E5BASE
emptyfaok:F6NOFAFILE
vacuousok:D1NOFA
anywaiver:W7WRONG
substrwaiver:W7PFX
echoexcuse:W7MIXGAP"

  # Состав батареи сверяется со спекой §4 ПО ИМЕНАМ — иначе фраза «сверяется с этим
  # перечнем» в спеке остаётся обещанием, и мутант можно молча выбросить.
  if BATTERY_ENTRIES="${BATTERY_ENTRIES}" python3 - "${SPEC}" <<'PY'
import os, re, sys
from pathlib import Path

lines = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
para, grab = [], False
for line in lines:
    if line.startswith("**Батарея"):
        grab = True
    if grab:
        if not line.strip():
            break
        para.append(line)
if not para:
    print("абзац «Батарея» в §4 не найден")
    sys.exit(1)
spec_names = set(re.findall(r"`([a-z]+)` \(", "\n".join(para)))
code_names = {e.split(":", 1)[0] for e in os.environ["BATTERY_ENTRIES"].splitlines() if e.strip()}
if spec_names != code_names:
    print(f"ONLY_SPEC {sorted(spec_names - code_names)}")
    print(f"ONLY_CODE {sorted(code_names - spec_names)}")
    sys.exit(1)
print(f"BATTERY_SPEC_NAMES={len(spec_names)}")
PY
  then
    echo "PASS  СПЕКА⇄БАТАРЕЯ: состав мутантов совпал по именам"
  else
    echo "FAIL  СПЕКА⇄БАТАРЕЯ: состав мутантов §4 расходится с кодом батареи"
    bad=$((bad + 1))
  fi
  total=$((total + 1))

  while IFS= read -r entry; do
    [ -n "${entry}" ] || continue
    name="${entry%%:*}"; scen="${entry##*:}"
    env -u FIXTURES_REG REVIEW_FA_MUTANT="${name}" BARRIER="${ref}" CI="${wf}" bash "${SELF}" > "${d}/${name}.log" 2>&1
    rc=$?
    total=$((total + 1))
    # Ловля должна быть ИМЕННО названным сценарием: `grep -q "${scen}"` по всему логу
    # зеленел от строки `PASS  <scen>` — имя сценария печатается при ЛЮБОМ исходе.
    # Якорь `^FAIL  <scen> ` требует, чтобы красным стал именно он; хвостовой разделитель
    # обязателен, иначе `B4DEAD` матчит `B4DEADPFX` — тот же подстрочный класс, что Б-1.
    if [ "${rc}" -ne 0 ] && grep -qE "^FAIL  ${scen}( |\$)" "${d}/${name}.log"; then
      echo "PASS  ${name} → пойман сценарием ${scen} (exit=${rc})"
    else
      echo "FAIL  ${name} → exit=${rc}, ожидался красный ${scen}"
      grep -E '^(FAIL|VERDICT|SETUP)' "${d}/${name}.log" | head -10 | sed 's/^/      ↳ /'
      bad=$((bad + 1))
    fi
  done <<EOF_BATTERY
${BATTERY_ENTRIES}
EOF_BATTERY

  if [ "${bad}" -gt 0 ]; then
    echo "BATTERY: FAIL (${bad} из ${total})"
    return 1
  fi
  echo "BATTERY: PASS (${total}/${total})"
  return 0
}

if [ "${1:-}" = "--battery" ]; then
  run_battery
  exit $?
fi

[ -f "${SPEC}" ] || die "спеки нет: ${SPEC}. Состав §4 сверять не с чем."
[ -f "${BARRIER}" ] || die "барьера нет: ${BARRIER}. Проба НЕ имеет права выносить вердикт: 127 от bash неотличим от честного отказа гейта."
bash -n "${BARRIER}" 2>/dev/null || die "барьер не парсится — его отказ неотличим от ошибки интерпретатора."

echo "── M-66 RED: review-fa FA echo attestation ──"
echo "барьер: ${BARRIER}"
echo "спека:  ${SPEC}"
echo "ci:     ${CI}"
echo

positive_control
echo "PASS  POSITIVE-CONTROL: заведомо годная фикстура даёт exit=0 до kill-сценариев"
echo

scenario_D1DOC
scenario_D1NOREV
scenario_D1REVPAIR
scenario_D1NOFA
scenario_D1NOFAW
scenario_D1NOFANOREV
scenario_M2KNOWN
scenario_M2UNKNOWN
scenario_M2VENUE
scenario_S3ADDED
scenario_S3NAMED
scenario_S3GHOST
scenario_S3BAREID
scenario_B4LIVE
scenario_B4CROSSCUT
scenario_B4FOREIGN
scenario_B4DEAD
scenario_B4UNION
scenario_B4DEADPFX
scenario_B4LIVETAIL
scenario_B4EMPTYU
scenario_E5EVENT
scenario_E5BASE
scenario_E5OK
scenario_F6NOFAFILE
scenario_F6NODIR
scenario_F6NOREV
scenario_F6SETUPGUARD
scenario_W7WRONG
scenario_W7MIXGAP
scenario_W7MIXW
scenario_W7EMPTY
scenario_W7SHORT
scenario_W7EXACT
scenario_W7PFX
scenario_W8NONEEDS
scenario_W8NOIF
scenario_W8NOCALL
scenario_W8OK
scenario_W8CMTONLY

echo
DECL_NAMES="$(printf '%s\n' "${MANIFEST}" | grep '|' | cut -d'|' -f1 | sort -u)"
RUN_NAMES="$(printf '%s\n' "${EXECUTED}" | grep . | sort -u)"
MISS="$(comm -23 <(printf '%s\n' "${DECL_NAMES}") <(printf '%s\n' "${RUN_NAMES}") | tr '\n' ' ')"
EXTRA="$(comm -13 <(printf '%s\n' "${DECL_NAMES}") <(printf '%s\n' "${RUN_NAMES}") | tr '\n' ' ')"
if [ -n "${MISS// /}" ] || [ -n "${EXTRA// /}" ]; then
  [ -n "${MISS// /}" ] && meta_fail "МАНИФЕСТ: объявлены, НЕ исполнены: ${MISS}"
  [ -n "${EXTRA// /}" ] && meta_fail "МАНИФЕСТ: исполнены, НЕ объявлены: ${EXTRA}"
else
  meta_pass "МАНИФЕСТ ⇄ исполнение: $(printf '%s\n' "${RUN_NAMES}" | grep -c .) сценариев, состав совпал"
fi

if M66_MANIFEST="${MANIFEST}" python3 - "${SPEC}" <<'PY'; then
import os, re, sys
from pathlib import Path

spec = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
inside = False
rows = []
for line in spec:
    if line.startswith("| ось | значение | сценарий | ожидаемый исход |"):
        inside = True
        continue
    if inside:
        if line.startswith("|---"):
            continue
        if not line.startswith("|"):
            break
        parts = [re.sub(r"\s+", " ", p.strip()) for p in line.strip().strip("|").split("|")]
        if len(parts) == 4:
            rows.append(tuple(parts))

manifest = []
for line in os.environ["M66_MANIFEST"].splitlines():
    if not line.strip():
        continue
    parts = [re.sub(r"\s+", " ", p.strip()) for p in line.split("|", 4)]
    if len(parts) != 5:
        print(f"bad manifest row: {line}")
        sys.exit(1)
    manifest.append(tuple(parts[1:]))

only_spec = sorted(set(rows) - set(manifest))
only_manifest = sorted(set(manifest) - set(rows))
if not rows:
    print("таблица §4 не разобрана")
    sys.exit(1)
if only_spec or only_manifest:
    print(f"spec rows={len(rows)} manifest rows={len(manifest)}")
    for row in only_spec:
        print("ONLY_SPEC " + " | ".join(row))
    for row in only_manifest:
        print("ONLY_MANIFEST " + " | ".join(row))
    sys.exit(1)
print(f"SPEC_ROWS={len(rows)}")
PY
  meta_pass "СПЕКА⇄МАНИФЕСТ: $(M66_MANIFEST="${MANIFEST}" python3 - "${SPEC}" <<'PY'
import os, re, sys
from pathlib import Path
inside=False; n=0
for line in Path(sys.argv[1]).read_text(encoding="utf-8").splitlines():
    if line.startswith("| ось | значение | сценарий | ожидаемый исход |"):
        inside=True; continue
    if inside:
        if line.startswith("|---"): continue
        if not line.startswith("|"): break
        n += 1
print(n)
PY
) строк §4 совпали в обе стороны"
else
  meta_fail "СПЕКА⇄МАНИФЕСТ: состав §4 расходится"
fi

TOTAL="$(printf '%s\n' "${DECL_NAMES}" | grep -c .)"
echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} нарушений; ${PASSED}/${TOTAL} сценариев прошли)"
  exit 1
fi
echo "VERDICT: PASS (${PASSED}/${TOTAL})"
