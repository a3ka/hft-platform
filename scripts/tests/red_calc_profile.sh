#!/usr/bin/env bash
# Проба барьера `scripts/check_calc_profile.sh` (M-94 задача 9, `П-032` п.3; спека §3.8, I-9).
#
# Каждый мир — НАСТОЯЩИЙ git-репозиторий: база (профиль v1 + `docs/PENDING-SIGNATURE.md` с
# заголовком `## П-032`) и коммиты мира; барьер зовётся ТОЙ ЖЕ формой, что в CI
# (`EVENT_NAME=pull_request PR_BASE_SHA=<база>`). Ожидание — код возврата.
#
# Батарея (`--battery`, гоняется и по умолчанию): эталон обязан пройти ВСЕ миры; каждый мутант
# барьера обязан уронить хотя бы один мир — иначе мир его не пиннит, и проба плацебо.
#
# Число сценариев НЕ заявляется — считается и печатается. Песочницы — в одном каталоге под
# `trap EXIT`; после прогона печатается число оставшихся каталогов (класс «10 400 каталогов /tmp»).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BARRIER="$ROOT/scripts/check_calc_profile.sh"
SANDBOX="$(mktemp -d)"; trap 'rm -rf "$SANDBOX"' EXIT
FAILS=0; N=0

V1=$'# профиль\nCALC_PROFILE_VERSION=1\nGATEWAY_BANDS=0.015,0.3\nGATEWAY_HEATMAP_WINDOW=0.001\n'

# ── песочница: база с профилем v1 и подписью П-032 ──────────────────────────────────
mk_repo() { # <каталог> → 0 при состоявшейся подготовке
  local d="$1"
  mkdir -p "$d" && cd "$d" || return 1
  git init -q -b main . && git config user.email p@x && git config user.name probe || return 1
  mkdir -p config/calc-profile docs
  printf '%s' "$V1" > config/calc-profile/active.env
  printf '# Очередь\n\n## П-032 — носитель профиля\n' > docs/PENDING-SIGNATURE.md
  git add -A && git commit -qm base || return 1
  git rev-parse HEAD > "$d.base"
  # страж подготовки: база несёт профиль и подпись
  git cat-file -e HEAD:config/calc-profile/active.env && git show HEAD:docs/PENDING-SIGNATURE.md | grep -q '^## П-032'
}
commit() { # <subject> <body> — фиксирует ВСЁ изменённое в песочнице (это не репозиторий проекта)
  git add -A && git commit -q --allow-empty -m "$1" -m "$2"
}
set_kv() { sed -i "s|^$1=.*|$1=$2|" "$3"; }

run_barrier() { # <барьер> <каталог> → код возврата
  ( cd "$2" && EVENT_NAME=pull_request PR_BASE_SHA="$(cat "$2.base")" bash "$1" >/dev/null 2>&1 )
}

# ── миры: имя | ожидаемый код | шаги ─────────────────────────────────────────────────
W_NAMES=(); W_RC=()
world() { W_NAMES+=("$1"); W_RC+=("$2"); }

w_untouched()   { echo x > other.txt; commit "feat: прочее" ""; }
w_good()        { set_kv GATEWAY_BANDS 0.015,0.3,0.6 config/calc-profile/active.env; set_kv CALC_PROFILE_VERSION 2 config/calc-profile/active.env; commit "feat: полосы" "CALC-PROFILE-DECISION: П-032"; }
w_no_token()    { set_kv GATEWAY_BANDS 0.015,0.3,0.6 config/calc-profile/active.env; set_kv CALC_PROFILE_VERSION 2 config/calc-profile/active.env; commit "feat: полосы" "без ссылки"; }
w_subject_only(){ set_kv GATEWAY_BANDS 0.015,0.3,0.6 config/calc-profile/active.env; set_kv CALC_PROFILE_VERSION 2 config/calc-profile/active.env; commit "CALC-PROFILE-DECISION: П-032" ""; }
w_token_in_file(){ set_kv GATEWAY_BANDS 0.015,0.3,0.6 config/calc-profile/active.env; set_kv CALC_PROFILE_VERSION 2 config/calc-profile/active.env; echo '# CALC-PROFILE-DECISION: П-032' >> config/calc-profile/active.env; commit "feat: полосы" "самоавторизация через файл"; }
w_unknown_p()   { set_kv GATEWAY_BANDS 0.015,0.3,0.6 config/calc-profile/active.env; set_kv CALC_PROFILE_VERSION 2 config/calc-profile/active.env; commit "feat: полосы" "CALC-PROFILE-DECISION: П-999"; }
w_same_ver()    { set_kv GATEWAY_BANDS 0.015,0.3,0.6 config/calc-profile/active.env; commit "feat: полосы" "CALC-PROFILE-DECISION: П-032"; }
w_lower_ver()   { set_kv GATEWAY_BANDS 0.015,0.3,0.6 config/calc-profile/active.env; set_kv CALC_PROFILE_VERSION 0 config/calc-profile/active.env; commit "feat: полосы" "CALC-PROFILE-DECISION: П-032"; }
w_bad_ver()     { set_kv GATEWAY_BANDS 0.015,0.3,0.6 config/calc-profile/active.env; set_kv CALC_PROFILE_VERSION v2 config/calc-profile/active.env; commit "feat: полосы" "CALC-PROFILE-DECISION: П-032"; }
w_comment_tok() { echo '# пояснение' >> config/calc-profile/active.env; commit "docs: комментарий" "CALC-PROFILE-DECISION: П-032"; }
w_comment_none(){ echo '# пояснение' >> config/calc-profile/active.env; commit "docs: комментарий" ""; }
w_next_good()   { printf '%s' "$V1" > config/calc-profile/next.env; set_kv CALC_PROFILE_VERSION 2 config/calc-profile/next.env; set_kv GATEWAY_BANDS 0.015 config/calc-profile/next.env; commit "feat: прогрев" "CALC-PROFILE-DECISION: П-032"; }
w_next_eq()     { printf '%s' "$V1" > config/calc-profile/next.env; set_kv GATEWAY_BANDS 0.015 config/calc-profile/next.env; commit "feat: прогрев" "CALC-PROFILE-DECISION: П-032"; }
w_switch()      { w_next_good; cp config/calc-profile/next.env config/calc-profile/active.env; git rm -q config/calc-profile/next.env; commit "feat: переключение" "CALC-PROFILE-DECISION: П-032"; }
w_del_active()  { git rm -q config/calc-profile/active.env; commit "chore: удалить" "CALC-PROFILE-DECISION: П-032"; }
w_bad_then_good(){ w_no_token; echo '# ok' >> config/calc-profile/active.env; commit "docs" "CALC-PROFILE-DECISION: П-032"; }
w_revert_pair() { cp config/calc-profile/active.env /tmp/.m94keep.$$; set_kv GATEWAY_BANDS 0.6 config/calc-profile/active.env; set_kv CALC_PROFILE_VERSION 2 config/calc-profile/active.env; commit "x" ""; cp /tmp/.m94keep.$$ config/calc-profile/active.env; rm -f /tmp/.m94keep.$$; commit "revert x" ""; }
w_p_same_commit(){ printf '\n## П-040 — новая подпись\n' >> docs/PENDING-SIGNATURE.md; set_kv GATEWAY_BANDS 0.6 config/calc-profile/active.env; set_kv CALC_PROFILE_VERSION 2 config/calc-profile/active.env; commit "feat" "CALC-PROFILE-DECISION: П-040"; }

world untouched 0;     world good 0;          world no_token 1;     world subject_only 1
world token_in_file 1; world unknown_p 1;     world same_ver 1;     world lower_ver 1
world bad_ver 1;       world comment_tok 0;   world comment_none 1; world next_good 0
world next_eq 1;       world switch 0;        world del_active 1;   world bad_then_good 1
world revert_pair 1;   world p_same_commit 0

run_suite() { # <барьер> <метка> → печатает FAIL-миры, возвращает число провалов
  local b="$1" tag="$2" i name want got fails=0 d
  for i in "${!W_NAMES[@]}"; do
    name="${W_NAMES[$i]}"; want="${W_RC[$i]}"
    d="$SANDBOX/$tag-$name"
    if ! ( mk_repo "$d" >/dev/null 2>&1 && cd "$d" && "w_$name" >/dev/null 2>&1 ); then
      echo "FAIL  [$tag] $name: SETUP не состоялся"; fails=$((fails + 1)); continue
    fi
    run_barrier "$b" "$d"; got=$?
    if [ "$got" -ne "$want" ]; then
      [ "$tag" = эталон ] && echo "FAIL  [$tag] $name: ожидался код $want, получен $got"
      fails=$((fails + 1))
    elif [ "$tag" = эталон ]; then
      echo "pass  [$tag] $name (код $got)"
    fi
  done
  return "$fails"
}

# ── 1. база события: fail-closed (три формы недостоверной базы + неизвестное событие) ──
d="$SANDBOX/base"; ( mk_repo "$d" >/dev/null 2>&1 ) || { echo "FAIL  SETUP base"; exit 1; }
base_case() { # <метка> <env…> — барьер ОБЯЗАН отказать
  N=$((N + 1)); local tag="$1"; shift
  if ( cd "$d" && env "$@" bash "$BARRIER" >/dev/null 2>&1 ); then
    echo "FAIL  база: $tag — барьер прошёл"; FAILS=$((FAILS + 1))
  else echo "pass  база: $tag ⇒ отказ"; fi
}
base_case "событие не задано"     -u EVENT_NAME
base_case "неизвестное событие"   EVENT_NAME=schedule
base_case "пустая база PR"        EVENT_NAME=pull_request PR_BASE_SHA=
base_case "zero-SHA"              EVENT_NAME=push PUSH_BEFORE=0000000000000000000000000000000000000000
base_case "базы нет в истории"    EVENT_NAME=pull_request PR_BASE_SHA=1234567890123456789012345678901234567890

# ── 2. эталон — все миры ─────────────────────────────────────────────────────────────
N=$((N + ${#W_NAMES[@]}))
run_suite "$BARRIER" эталон; r=$?; FAILS=$((FAILS + r))

# ── 3. батарея мутантов: каждый обязан уронить хотя бы один мир ─────────────────────
mutant() { # <метка> <sed-выражение>
  N=$((N + 1))
  local m="$SANDBOX/mut-$1.sh"
  sed -E "$2" "$BARRIER" > "$m"
  if cmp -s "$m" "$BARRIER"; then echo "FAIL  мутант $1: SETUP — правка не применилась"; FAILS=$((FAILS + 1)); return; fi
  run_suite "$m" "$1" >/dev/null; local caught=$?
  if [ "$caught" -gt 0 ]; then echo "pass  мутант $1 пойман ($caught мир.)"
  else echo "FAIL  мутант $1 НЕ пойман ни одним миром"; FAILS=$((FAILS + 1)); fi
}
mutant subject_counts   "s/--format='%b'/--format='%B'/"
mutant no_p_check       's/^  elif ! git show "\$c:\$PS".*$/  elif false; then/'
mutant no_version_check 's/if \[ "\$vnew" -le "\$vold" \]; then/if false; then/'
mutant last_commit_only 's/for c in \$\(git rev-list --reverse "\$\{BASE\}..HEAD"\); do/for c in $(git rev-list --reverse "${BASE}..HEAD" | tail -1); do/'
mutant allow_del_active 's/^      \[ "\$f" = "\$ACTIVE" \] && bad .*$/      :/'
mutant comment_needs_bump 's/^    \[ "\$new_kv" = "\$old_kv" \] && continue.*$/    :/'
mutant ver_vs_zero      's/vold=\$\(ver_of "\$parent" "\$ACTIVE"\); ref=/vold=0; ref=/'

left=$(find "$SANDBOX" -mindepth 1 -maxdepth 1 -type d | wc -l)
echo "сценариев: $N; провалов: $FAILS; каталогов песочницы до уборки: $left (убираются trap EXIT)"
if [ "$FAILS" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; fi
echo "VERDICT: FAIL"; exit 1
