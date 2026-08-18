#!/usr/bin/env bash
# red_milestone_shape.sh — проба барьера `check_milestone_shape.sh`.
#
# Проба обязана быть КРАСНОЙ против обманных стабов и ЗЕЛЁНОЙ против честной реализации
# (`docs/workflow/harness-track.md` §5). Каждый сценарий несёт setup-guard: проба, молча
# тестирующая не тот сценарий, — плацебо самой себя (`testing.md`, целостность гейта, св. 3).
#
# УБОРКА: всё временное живёт под ОДНИМ корнем `$SBOX`, снимаемым `trap EXIT` целиком, и проба
# ПЕЧАТАЕТ ЧИСЛО остатка. Класс, ради которого: 10 400 каталогов `/tmp/red-freeze-*` и диск на
# 100 %. Первая редакция этой пробы держала реестр отдельных путей и чистила только каталоги
# (`-d`) — собственный замер показал «остаточных 3» (два стаба и out-файл), и конструкция была
# заменена на единый корень. Замер уборки в выводе — не украшение: он и поймал эту течь.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BARRIER="${BARRIER_OVERRIDE:-$ROOT/scripts/check_milestone_shape.sh}"
# ВСЁ временное — внутри ОДНОГО корня. Реестр отдельных путей отвергнут замером: первая
# редакция держала список и чистила только каталоги (`-d`), из-за чего стабы и out-файл
# переживали уборку («остаточных 3» в собственном выводе пробы), а вложенный self-test плодил
# свои. Один корень убирается целиком и корректно при любой вложенности.
SBOX="$(mktemp -d /tmp/red-mshape-root-XXXXXX)"
REGISTRY="$SBOX/registry"; : > "$REGISTRY"
OUT="$SBOX/out"
PASS=0; FAIL=0

cleanup() {
  rm -rf "$SBOX"
  local leaked
  leaked=$(find /tmp -maxdepth 1 -name 'red-mshape-*' 2>/dev/null | wc -l)
  echo "уборка: корень песочниц удалён; остаточных /tmp/red-mshape-*: $leaked"
  [ "$leaked" -eq 0 ] || echo "ВНИМАНИЕ: проба течёт — $leaked объектов осталось" >&2
}
trap cleanup EXIT

ok()   { PASS=$((PASS + 1)); echo "  PASS: $*"; }
bad()  { FAIL=$((FAIL + 1)); echo "  FAIL: $*" >&2; }

# Полная спека — эталон формы.
full_spec() {
  cat <<'EOF'
# M-99 — тестовая спека
## Objective
Цель.
## Allowed paths
| путь | кто |
## §Tasks
| # | Status |
## Acceptance
`scripts/verify_M-99.sh`
EOF
}

# Песочница: git-репозиторий с базой и одним коммитом поверх.
sandbox() {
  local d; d="$(mktemp -d "$SBOX/sandbox-XXXXXX")"
  git -C "$d" init -q
  git -C "$d" config user.email t@t; git -C "$d" config user.name t
  mkdir -p "$d/milestones" "$d/scripts"
  cp "$BARRIER" "$d/scripts/check_milestone_shape.sh"
  chmod +x "$d/scripts/check_milestone_shape.sh"
  echo seed > "$d/seed.txt"
  git -C "$d" add -A >/dev/null; git -C "$d" commit -qm base
  echo "$d"
}

run_barrier() {  # $1=dir  → печатает exit-код
  local d="$1" base
  base="$(git -C "$d" rev-parse HEAD~1 2>/dev/null || git -C "$d" rev-parse HEAD)"
  ( cd "$d" && EVENT_NAME=pull_request PR_BASE_SHA="$base" \
      bash scripts/check_milestone_shape.sh >"$OUT" 2>&1; echo $? )
}

scenario() {  # $1=имя  $2=ожидаемый_код  $3=тело_спеки_или_MISSING  $4=режим(add|modify|none)
  local name="$1" want="$2" body="$3" mode="$4"
  local d; d="$(sandbox)"
  case "$mode" in
    add)
      printf '%s\n' "$body" > "$d/milestones/M-99-probe.md"
      git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "add spec" ;;
    modify)
      printf '%s\n' "$body" > "$d/milestones/M-98-old.md"
      git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "seed old spec"
      echo "правка" >> "$d/milestones/M-98-old.md"
      git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "modify spec" ;;
    none)
      echo x > "$d/other.txt"
      git -C "$d" add -A >/dev/null; git -C "$d" commit -qm "unrelated" ;;
  esac
  # SETUP-GUARD: сценарий обязан состояться. Для add — файл ДОЛЖЕН числиться добавленным.
  if [ "$mode" = add ]; then
    git -C "$d" diff --diff-filter=A --name-only HEAD~1 HEAD -- 'milestones/M-*.md' \
      | grep -q . || { bad "$name — SETUP НЕ СОСТОЯЛСЯ: файл не числится добавленным"; return; }
  fi
  local got; got="$(run_barrier "$d")"
  if [ "$got" = "$want" ]; then ok "$name (exit=$got)"; else
    bad "$name — ожидался exit=$want, получен exit=$got"; sed -n '1,6p' "$OUT" >&2
  fi
}

echo "=== ЧЕСТНАЯ РЕАЛИЗАЦИЯ: позитивный контроль + отказы ==="
scenario "полная спека принимается"                    0 "$(full_spec)"                                       add
scenario "нет Allowed paths → отказ"                   1 "$(full_spec | grep -v 'Allowed paths')"             add
scenario "нет Objective → отказ"                       1 "$(full_spec | grep -v '## Objective')"              add
scenario "нет §Tasks → отказ"                          1 "$(full_spec | grep -v '## §Tasks')"                 add
scenario "нет Acceptance → отказ"                      1 "$(full_spec | grep -v '## Acceptance')"             add
scenario "три решётки (### Objective) принимаются"     0 "$(full_spec | sed 's/^## /### /')"                  add
scenario "ИЗМЕНЁННАЯ неполная спека НЕ трогается"      0 "# M-98 — старая"                                    modify
scenario "нет новых спек — проверять нечего"           0 ""                                                    none

echo "=== FAIL-CLOSED SETUP (барьер зовут не так, как зовёт CI) ==="
d="$(sandbox)"
got="$( cd "$d" && bash scripts/check_milestone_shape.sh >/dev/null 2>&1; echo $? )"
[ "$got" = 1 ] && ok "пустой EVENT_NAME → отказ (exit=1)" || bad "пустой EVENT_NAME: ожидался 1, получен $got"
got="$( cd "$d" && EVENT_NAME=pull_request PR_BASE_SHA=0000000000000000000000000000000000000000 \
        bash scripts/check_milestone_shape.sh >/dev/null 2>&1; echo $? )"
[ "$got" = 1 ] && ok "zero-SHA база → отказ (exit=1)" || bad "zero-SHA: ожидался 1, получен $got"
got="$( cd "$d" && EVENT_NAME=pull_request PR_BASE_SHA=deadbeefdeadbeefdeadbeefdeadbeefdeadbeef \
        bash scripts/check_milestone_shape.sh >/dev/null 2>&1; echo $? )"
[ "$got" = 1 ] && ok "несуществующая база → отказ (exit=1)" || bad "нет базы: ожидался 1, получен $got"

echo "=== АНТИ-ПЛАЦЕБО: обманные стабы обязаны быть ПОЙМАНЫ ==="
# Стаб 1 — «всегда успех» (классический no-op барьер).
stub1="$(mktemp "$SBOX/stub1-XXXXXX.sh")"
printf '#!/usr/bin/env bash\nexit 0\n' > "$stub1"; chmod +x "$stub1"
d="$(sandbox)"; printf '%s\n' "$(full_spec | grep -v 'Allowed paths')" > "$d/milestones/M-99-probe.md"
git -C "$d" add -A >/dev/null; git -C "$d" commit -qm add
cp "$stub1" "$d/scripts/check_milestone_shape.sh"
got="$(run_barrier "$d")"
[ "$got" = 0 ] && ok "стаб «всегда 0» пойман бы: на спеке без Allowed paths он даёт 0 вместо 1" \
               || bad "стаб «всегда 0» повёл себя неожиданно (exit=$got)"

# Стаб 2 — «отказ 127» (барьера нет / не исполняется). Отличается от честного отказа кодом.
stub2="$(mktemp "$SBOX/stub2-XXXXXX.sh")"
printf '#!/usr/bin/env bash\nexit 127\n' > "$stub2"; chmod +x "$stub2"
d="$(sandbox)"; printf '%s\n' "$(full_spec)" > "$d/milestones/M-99-probe.md"
git -C "$d" add -A >/dev/null; git -C "$d" commit -qm add
cp "$stub2" "$d/scripts/check_milestone_shape.sh"
got="$(run_barrier "$d")"
[ "$got" = 127 ] && ok "стаб «127» отличим от честного отказа (1) — страж не путает их" \
                 || bad "стаб «127» дал exit=$got"

# ── НАСТОЯЩИЙ анти-плацебо: проба ЦЕЛИКОМ против стаба обязана вернуть FAIL ──────────
# Сценарии выше констатируют поведение стаба; этого мало. Проба доказывает свою силу только
# тем, что САМА краснеет, когда барьер подменён. Рекурсия отсекается флагом.
if [ -z "${MSHAPE_SELFTEST:-}" ]; then
  echo "=== САМОПРОВЕРКА: проба против стаба «всегда 0» обязана дать VERDICT: FAIL ==="
  selfstub="$(mktemp "$SBOX/selfstub-XXXXXX.sh")"
  printf '#!/usr/bin/env bash\nexit 0\n' > "$selfstub"; chmod +x "$selfstub"
  if MSHAPE_SELFTEST=1 BARRIER_OVERRIDE="$selfstub" bash "$0" >/dev/null 2>&1; then
    bad "САМОПРОВЕРКА: проба ЗЕЛЁНАЯ против стаба «всегда 0» — она ничего не пиннит"
  else
    ok "проба краснеет против подменённого барьера (её сценарии реально давят)"
  fi
fi

echo
echo "PASS=$PASS FAIL=$FAIL (сценариев: $((PASS + FAIL)))"
[ "$FAIL" -eq 0 ] && { echo "VERDICT: PASS"; exit 0; } || { echo "VERDICT: FAIL"; exit 1; }
