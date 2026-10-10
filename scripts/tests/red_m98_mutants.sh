#!/usr/bin/env bash
# red_m98_mutants.sh — ИСПОЛНЯЕМАЯ таблица мутаций M-98 §8 (закрытие C-297 B2).
#
# ЧТО ДОКАЗЫВАЕТ. Различающую силу RED `red_m98_switch_provenance` (s0…s4): против БАЗОВОГО кода
# красны все пять; против ПРОТОТИПА по спеке §3 — зелены все пять; против каждого мутанта
# краснеет РОВНО объявленный набор сценариев. Это свойство НАБОРА против эталонных реализаций,
# а не проверка продуктовой реализации — её судит обычный прогон RED.
#
# КАК. Временный worktree на HEAD (берутся ТЕКУЩИЕ тесты), в него кладётся `lib.rs` транспорта с
# ЗАКРЕПЛЁННОЙ базы `a660b3a7` (патч прототипа написан против неё и потому применим и после того,
# как engine-dev изменит продуктовый `lib.rs`), затем `scripts/tests/m98/prototype.patch`.
# Мутант выбирается переменной `M98_MUT` внутри прототипа. Продуктовый код не трогается.
#
# FAIL-CLOSED: патч не применился / тест-бинарь не собрался / исполнено не ровно 5 сценариев /
# набор красных ≠ объявленному — FAIL. Кэш сборки — внутри временного дерева, удаляется на выходе.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BASE_REV="a660b3a7b78b116a0a98bc7c9d9eb1e03153725c"
PATCH="${ROOT}/scripts/tests/m98/prototype.patch"
LIB="crates/gateway-serve/src/lib.rs"
FAIL=0
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s\n' "$1"; FAIL=$((FAIL + 1)); }

[ -f "${PATCH}" ] || { echo "FAIL  нет ${PATCH}"; echo "VERDICT: FAIL"; exit 1; }
git -C "${ROOT}" cat-file -e "${BASE_REV}^{commit}" 2>/dev/null \
  || { echo "FAIL  базовой ревизии ${BASE_REV} нет в клоне (shallow?) — воспроизведение невозможно"; echo "VERDICT: FAIL"; exit 1; }

W="$(mktemp -d /tmp/red-m98-mut-XXXXXX)" || exit 2
cleanup() { git -C "${ROOT}" worktree remove --force "${W}/t" >/dev/null 2>&1; rm -rf "${W}"; }
trap cleanup EXIT
git -C "${ROOT}" worktree add -q --detach "${W}/t" HEAD || { echo "FAIL  worktree"; echo "VERDICT: FAIL"; exit 1; }
T="${W}/t"
export CARGO_TARGET_DIR="${W}/target"

base_lib() { git -C "${ROOT}" show "${BASE_REV}:${LIB}" > "${T}/${LIB}"; }

# $1=имя мира, $2=M98_MUT, $3=ожидаемые красные (через пробел, отсортированы), $4=patch|base
world() {
  local name="$1" mut="$2" want="$3" mode="$4" out got n
  base_lib
  if [ "${mode}" = "patch" ]; then
    if ! (cd "${T}" && patch -s -p1 --dry-run < "${PATCH}" >/dev/null && patch -s -p1 < "${PATCH}"); then
      fail "${name}: прототип не применился к ${LIB}@${BASE_REV:0:8} — мир не построен"; return
    fi
    grep -q 'm98-switch-catalog' "${T}/${LIB}" || { fail "${name}: SETUP — точки m98-switch-catalog в прототипе нет"; return; }
  fi
  out="$(cd "${T}" && M98_MUT="${mut}" cargo test -p gateway-serve --features testing \
           --test red_m98_switch_provenance 2>&1)"
  n="$(printf '%s\n' "${out}" | grep -cE '^test s[0-4]_[a-z0-9_]+ \.\.\. (ok|FAILED)$')"
  if [ "${n}" -ne 5 ]; then
    fail "${name}: исполнено сценариев ${n}, ожидалось 5 — мир не судился (сборка?)"
    printf '%s\n' "${out}" | grep -E '^error' | head -3
    return
  fi
  got="$(printf '%s\n' "${out}" | sed -nE 's/^test (s[0-4])_[a-z0-9_]+ \.\.\. FAILED$/\1/p' | sort | tr '\n' ' ' | sed 's/ $//')"
  if [ "${got}" = "${want}" ]; then pass "${name}: красны «${got:-нет}» — как объявлено"
  else fail "${name}: красны «${got:-нет}», объявлено «${want:-нет}»"; fi
}

world "базовый код a660b3a7"                         ""           "s0 s1 s2 s3 s4" base
world "прототип по §3"                               ""           ""               patch
world "мутант: провенанс без is_fresh/refresh"       "nofresh"    "s2 s3 s4"       patch
world "мутант: Err is_fresh/refresh проглочен"       "swallow"    "s3 s4"          patch
world "мутант: на SWITCH всегда (frozen, true)"      "alwaystrue" "s0 s1 s2"       patch
world "мутант: начало истории = 1 при усечении (C-297 B1)" "startone" "s1 s2"     patch

[ "${FAIL}" -eq 0 ] && { echo "VERDICT: PASS — 6 миров"; exit 0; }
echo "VERDICT: FAIL (провалов: ${FAIL})"; exit 1
