#!/usr/bin/env bash
# Проба барьера `scripts/check_roadmap_sync.sh` — АНТИ-ПЛАЦЕБО В ОБЕ СТОРОНЫ.
#
# Барьер, о котором известно только «он зелёный», не доказывает ничего: зелёным он бывает и
# оттого, что слеп. Предъявляются ОБА свойства:
#   · КРАСНЕЕТ, когда предмет закрыт (спека уехала в архив), а роадмап не тронут;
#   · МОЛЧИТ, когда close-out'а нет либо роадмап обновлён вместе с ним.
# Второе не менее важно первого: барьер, краснеющий на честной работе, будет обойдён.
#
# Каждый сценарий гоняет барьер ТОЙ ЖЕ ПРОВОДКОЙ, какой его зовёт CI (`EVENT_NAME`/
# `PUSH_BEFORE`), в отдельном git-репозитории. Барьер, проверенный не тем вызовом, каким его
# зовёт прод, не проверен (`testing.md` «Целостность гейта», свойство 1).
#
# Число сценариев НЕ ЗАЯВЛЯЕТСЯ цифрой в комментарии — оно СЧИТАЕТСЯ и печатается в итоге.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BARRIER="$ROOT/scripts/check_roadmap_sync.sh"
[ -x "$BARRIER" ] || { echo "SETUP НЕ СОСТОЯЛСЯ: барьер $BARRIER не исполняем — проба судила бы пустоту"; exit 1; }

PASS=0; FAIL=0
ok()   { printf 'pass  %s\n' "$*"; PASS=$((PASS+1)); }
nope() { printf 'FAIL  %s\n' "$*"; FAIL=$((FAIL+1)); }

SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT

# Роадмап с достаточным числом пунктов — иначе сработает проверка «выхолощен до заглушки»,
# и сценарий судил бы НЕ ТОТ отказ.
roadmap_body() {
  printf '# ROADMAP\n\n| № | Предмет | Состояние |\n|---|---|---|\n'
  printf '| 1 | первый | OPEN |\n| 2 | второй | OPEN |\n| 3 | третий | OPEN |\n'
  [ -n "${1:-}" ] && printf '| 4 | %s | OPEN |\n' "$1"
  return 0
}

# Собрать репозиторий: базовый коммит, затем «событийный» коммит по сценарию.
# Печатает `<exit> <BASE>`; при несостоявшемся setup — `99`.
run_case() {
  local do_archive="$1" do_roadmap="$2" dir base
  dir="$SANDBOX/case-$RANDOM$RANDOM"
  mkdir -p "$dir/scripts" "$dir/milestones" "$dir/docs/archive" || { echo 99; return; }
  git -C "$dir" init -q 2>/dev/null || { echo 99; return; }
  git -C "$dir" config user.email t@t.local
  git -C "$dir" config user.name t
  cp "$BARRIER" "$dir/scripts/check_roadmap_sync.sh"

  # БАЗА: роадмап и спека милестоуна существуют, ничего не закрыто.
  roadmap_body > "$dir/docs/ROADMAP.md"
  printf '# M-99 — предмет пробы\n' > "$dir/milestones/M-99-probe.md"
  git -C "$dir" add -A >/dev/null 2>&1
  git -C "$dir" commit -qm base >/dev/null 2>&1
  base="$(git -C "$dir" rev-parse HEAD)"

  # СОБЫТИЕ.
  if [ "$do_archive" = "yes" ]; then
    git -C "$dir" mv milestones/M-99-probe.md docs/archive/M-99-probe.md >/dev/null 2>&1
  else
    printf 'правка, не связанная с close-out\n' >> "$dir/milestones/M-99-probe.md"
  fi
  [ "$do_roadmap" = "yes" ] && roadmap_body "закрыт M-99" > "$dir/docs/ROADMAP.md"
  git -C "$dir" add -A >/dev/null 2>&1
  git -C "$dir" commit -qm event >/dev/null 2>&1

  ( cd "$dir" && EVENT_NAME=push PUSH_BEFORE="$base" bash scripts/check_roadmap_sync.sh >/dev/null 2>&1; echo "$?" )
}

expect() {
  local name="$1" want="$2" got="$3"
  if [ "$got" = "99" ]; then nope "$name — SETUP не состоялся, сценарий не судил предмет"
  elif [ "$got" = "$want" ]; then ok "$name"
  else nope "$name — ожидался exit=$want, получен exit=$got"; fi
}

echo "── КРАСНЕЕТ там, где обязан ─────────────────────────────────────────────────"
expect "close-out БЕЗ правки роадмапа" 1 "$(run_case yes no)"

echo
echo "── МОЛЧИТ там, где обязан (ложное красное так же вредно) ────────────────────"
expect "close-out С правкой роадмапа"        0 "$(run_case yes yes)"
expect "правка без close-out'а"              0 "$(run_case no no)"
expect "правка роадмапа без close-out'а"     0 "$(run_case no yes)"

echo
echo "── FAIL-CLOSED: «не могу проверить» ≠ «нечего проверять» ────────────────────"
mk_min() {
  local dir="$SANDBOX/fc-$RANDOM$RANDOM"
  mkdir -p "$dir/scripts" "$dir/docs"; git -C "$dir" init -q
  git -C "$dir" config user.email t@t.local; git -C "$dir" config user.name t
  cp "$BARRIER" "$dir/scripts/check_roadmap_sync.sh"
  roadmap_body > "$dir/docs/ROADMAP.md"
  git -C "$dir" add -A >/dev/null 2>&1; git -C "$dir" commit -qm base >/dev/null 2>&1
  printf '%s' "$dir"
}
d="$(mk_min)"
rc="$( cd "$d" && EVENT_NAME= PUSH_BEFORE= bash scripts/check_roadmap_sync.sh >/dev/null 2>&1; echo $? )"
expect "событие не задано → отказ" 1 "$rc"

d="$(mk_min)"
rc="$( cd "$d" && EVENT_NAME=push PUSH_BEFORE=0000000000000000000000000000000000000000 bash scripts/check_roadmap_sync.sh >/dev/null 2>&1; echo $? )"
expect "zero-SHA (force-push/новая ветка) → отказ" 1 "$rc"

d="$(mk_min)"
rc="$( cd "$d" && EVENT_NAME=push PUSH_BEFORE=deadbeefdeadbeefdeadbeefdeadbeefdeadbeef bash scripts/check_roadmap_sync.sh >/dev/null 2>&1; echo $? )"
expect "база вне истории → отказ" 1 "$rc"

echo
echo "── РОАДМАП НЕ ИМЕЕТ ПРАВА БЫТЬ ВЫХОЛОЩЕН ───────────────────────────────────"
# Без этой проверки барьер обходится тривиально: снести содержимое, оставить файл, и
# требование «тронут» будет удовлетворяться пустышкой.
d="$(mk_min)"; base="$(git -C "$d" rev-parse HEAD)"
printf '# ROADMAP\n' > "$d/docs/ROADMAP.md"
git -C "$d" add -A >/dev/null 2>&1; git -C "$d" commit -qm gut >/dev/null 2>&1
rc="$( cd "$d" && EVENT_NAME=push PUSH_BEFORE="$base" bash scripts/check_roadmap_sync.sh >/dev/null 2>&1; echo $? )"
expect "план выхолощен до заголовка → отказ" 1 "$rc"

d="$(mk_min)"; base="$(git -C "$d" rev-parse HEAD)"
git -C "$d" rm -q docs/ROADMAP.md >/dev/null 2>&1
git -C "$d" commit -qm drop >/dev/null 2>&1
rc="$( cd "$d" && EVENT_NAME=push PUSH_BEFORE="$base" bash scripts/check_roadmap_sync.sh >/dev/null 2>&1; echo $? )"
expect "роадмап снесён → отказ" 1 "$rc"

echo
echo "── ПРОД-ФОРМА: барьер на РЕАЛЬНОМ дереве репозитория ───────────────────────"
# Суррогат из песочницы не проверяет, что барьер применим к настоящему корпусу.
( cd "$ROOT" && EVENT_NAME=push PUSH_BEFORE="$(git rev-parse HEAD~1 2>/dev/null || git rev-parse HEAD)" bash scripts/check_roadmap_sync.sh >/dev/null 2>&1 )
rc=$?
if [ "$rc" -eq 0 ]; then ok "реальное дерево: барьер применим и молчит"
else nope "реальное дерево: exit=$rc — барьер непригоден к включению в CI"; fi

echo
echo "────────────────────────────────────────────────────────────────────────────"
echo "сценариев: $((PASS + FAIL))   pass=$PASS   FAIL=$FAIL"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL"; exit 1; fi
