#!/usr/bin/env bash
# red_gc_reclaim_args.sh — проба разбора аргументов `scripts/gc_worktrees.sh`.
#
# ЗАЧЕМ. `--reclaim` — единственный РАЗРУШИТЕЛЬНЫЙ путь в проекте, который запускают вручную
# и по памяти. Его страж («не трогай кэш дерева, молчавшего меньше Ч часов») стоит на
# сравнении `[ "$idle_h" -lt "$IDLE_H" ]`. Нечисловой порог роняет само сравнение, `[`
# возвращает не-ноль, ветка KEEP-CACHE не берётся — и управление уходит прямо в `rm -rf`.
# То есть при негодном аргументе страж не ужесточается, а ОТКЛЮЧАЕТСЯ.
#
# Замер на живом репозитории (R-055 Б-1, воспроизведён architect'ом 2026-08-12):
#   $ gc_worktrees.sh --reclaim-dry 3ч
#   [: 3ч: integer expression expected
#   WOULD-RECLAIM  hft-engine-dev-… — 8829MB, молчит 1ч
#   WOULD-RECLAIM  hft-tester-m62-r4 — 9465MB, молчит 0ч   ← агенты работали в этот момент
# Опечатка правдоподобна: шапка Usage сама печатала параметр как `[Ч]`.
#
# ЧТО ЗДЕСЬ ПРОВЕРЯЕТСЯ — обе стороны, а не только отказ:
#   негодный порог/флаг  ⇒ отказ ДО каких-либо удалений;
#   годный порог         ⇒ путь удаления РЕАЛЬНО достижим (иначе «ничего не снесено» зелено
#                          просто потому, что механизм мёртв — плацебо самой пробы).
#
# ФИКСТУРА — своя песочница на каждый сценарий: `git init` + bare origin + два worktree'а,
# у одного `target/` свежий, у второго состаренный `touch -d`. Обе стороны порога в одном
# прогоне. `pgrep` подменяется стабом: fail-closed страж «идёт сборка» обязан оставаться
# КОНСТАНТОЙ, иначе исход пробы зависит от того, компилирует ли кто-то на хосте в эту минуту
# (`testing.md`: гейт меряет свой инвариант, а не окружение).
#
# Анти-плацебо: против ДОФИКСНОГО скрипта проба обязана краснеть.
#   GC_UNDER_TEST=<путь к b228369-версии> bash scripts/tests/red_gc_reclaim_args.sh

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
GC="${GC_UNDER_TEST:-${ROOT}/scripts/gc_worktrees.sh}"
FAILED=0
RAN=0
EXPECT_SCENARIOS=11

pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
setup_fail() { echo "SETUP НЕ СОСТОЯЛСЯ  $*"; FAILED=$((FAILED + 1)); }

[ -f "$GC" ] || { echo "SETUP НЕ СОСТОЯЛСЯ: нет скрипта $GC"; exit 1; }

# ─── песочница ───────────────────────────────────────────────────────────────────────────
# Каждый worktree получает СВОЙ только-локальный коммит: иначе вторая фаза (штатный GC)
# сносит его целиком как «чистый, на origin, смержен», и `target/` исчезает не по причине
# reclaim'а — проба мерила бы не то, что обещает.
mk_sandbox() {
  local s
  s="$(mktemp -d /tmp/red-gcargs-XXXXXX)" || return 1
  git init -q --bare "$s/origin.git" >/dev/null 2>&1 || return 1
  git -c init.defaultBranch=main clone -q "$s/origin.git" "$s/repo" >/dev/null 2>&1 || return 1
  (
    cd "$s/repo" || exit 1
    git -c user.email=probe@local -c user.name=probe commit -q --allow-empty -m init
    git branch -M main >/dev/null 2>&1
    git push -q origin main >/dev/null 2>&1
    for w in wt-fresh wt-idle; do
      git worktree add -q -b "b-$w" "$s/$w" >/dev/null 2>&1
      git -C "$s/$w" -c user.email=probe@local -c user.name=probe \
        commit -q --allow-empty -m "local-only $w"
      mkdir -p "$s/$w/target"
      : >"$s/$w/target/marker"
    done
    touch -d '10 hours ago' "$s/wt-idle/target"
  ) || return 1
  # Стаб pgrep: «сборки нет» — константа для всех сценариев.
  mkdir -p "$s/bin"
  printf '#!/bin/sh\nexit 1\n' >"$s/bin/pgrep"
  chmod +x "$s/bin/pgrep"
  printf '%s\n' "$s"
}

# run_gc_with <скрипт> <sandbox> <args...> → печатает вывод, возвращает exit-код скрипта
run_gc_with() {
  local script="$1" s="$2"; shift 2
  ( cd "$s/repo" && PATH="$s/bin:$PATH" timeout 60 bash "$script" "$@" 2>&1 )
}
# run_gc <sandbox> <args...> — то же для скрипта под тестом
run_gc() {
  local s="$1"; shift
  run_gc_with "$GC" "$s" "$@"
}

# Проверка обеих сторон порога после ГОДНОГО прогона.
alive()  { [ -e "$1/target/marker" ]; }

# ─── setup-guard песочницы: фикстура обязана быть работоспособной ────────────────────────
S0="$(mk_sandbox)" || { echo "SETUP НЕ СОСТОЯЛСЯ: песочница не собралась"; exit 1; }
if ! ( cd "$S0/repo" && PATH="$S0/bin:$PATH" pgrep -x cargo >/dev/null 2>&1 ); then
  :
else
  setup_fail "стаб pgrep не подхватился — страж «идёт сборка» стал бы переменной, а не константой"
fi
if ! alive "$S0/wt-fresh" || ! alive "$S0/wt-idle"; then
  setup_fail "в песочнице нет target/marker — сценарии мерили бы отсутствие, созданное фикстурой"
fi

# ─── ПОЗИТИВНЫЙ КОНТРОЛЬ: разрушительный путь ДОСТИЖИМ ───────────────────────────────────
# Без него все ассерты «кэш цел» зелены и против скрипта, который не умеет удалять вовсе.
RAN=$((RAN + 1))
OUT="$(run_gc "$S0" --reclaim 0)"; RC=$?
if [ "$RC" -eq 0 ] && ! alive "$S0/wt-fresh" && ! alive "$S0/wt-idle"; then
  pass "CONTROL-REACHABLE: «--reclaim 0» реально сносит кэш обоих деревьев (путь достижим)"
else
  setup_fail "CONTROL-REACHABLE: «--reclaim 0» не снёс кэш (rc=$RC) — остальные сценарии \
проверяли бы неработающий механизм; «ничего не удалено» было бы зелёным по построению"
fi
rm -rf "$S0"

# ─── ПОРОГ РАБОТАЕТ В ОБЕ СТОРОНЫ ────────────────────────────────────────────────────────
RAN=$((RAN + 1))
S="$(mk_sandbox)" || setup_fail "песочница THRESHOLD-BOTH-SIDES"
OUT="$(run_gc "$S" --reclaim 3)"; RC=$?
if [ "$RC" -eq 0 ] && alive "$S/wt-fresh" && ! alive "$S/wt-idle"; then
  pass "THRESHOLD-BOTH-SIDES: свежий кэш сохранён, состаренный забран (порог 3ч различает)"
else
  fail "THRESHOLD-BOTH-SIDES: rc=$RC, свежий=$(alive "$S/wt-fresh" && echo цел || echo снесён), \
состаренный=$(alive "$S/wt-idle" && echo цел || echo снесён) — ожидалось цел/снесён"
fi
rm -rf "$S"

# ─── НЕГОДНЫЙ ПОРОГ: отказ ДО удаления ───────────────────────────────────────────────────
for bad in "3ч" "3h" "-1" "2.5"; do
  RAN=$((RAN + 1))
  S="$(mk_sandbox)" || { setup_fail "песочница BAD-THRESHOLD «$bad»"; continue; }
  OUT="$(run_gc "$S" --reclaim "$bad")"; RC=$?
  # Три условия конъюнкцией: ненулевой код И кэш цел И в выводе нет следа удаления.
  # Любое поодиночке проходит по неверной причине (напр. падение до разбора аргументов).
  if [ "$RC" -ne 0 ] && alive "$S/wt-fresh" && alive "$S/wt-idle" \
     && ! printf '%s' "$OUT" | grep -qE '^(RECLAIMED|WOULD-RECLAIM|REMOVED)'; then
    pass "BAD-THRESHOLD «$bad»: отказ (exit=$RC), кэш обоих деревьев цел"
  else
    fail "BAD-THRESHOLD «$bad»: exit=$RC, свежий=$(alive "$S/wt-fresh" && echo цел || echo СНЕСЁН), \
состаренный=$(alive "$S/wt-idle" && echo цел || echo СНЕСЁН). Страж fail-OPEN: негодный порог \
обязан ОСТАНАВЛИВАТЬ, а не отключать сравнение"
    printf '%s\n' "$OUT" | grep -E 'integer expression|RECLAIMED|WOULD-RECLAIM' | head -3 | sed 's/^/      ↳ /'
  fi
  rm -rf "$S"
done

# ─── НЕИЗВЕСТНЫЙ ФЛАГ: не смеет молча стать РЕАЛЬНЫМ прогоном ────────────────────────────
# `--dryrun`/`--reclaim-dry-run` — опечатки того же класса: `case` по первому аргументу их
# не ловил, MODE оставался `gc`, DRY — `0`, и «превью» оказывалось боевым сносом деревьев.
RAN=$((RAN + 1))
S="$(mk_sandbox)" || setup_fail "песочница UNKNOWN-FLAG"
OUT="$(run_gc "$S" --dryrun)"; RC=$?
if [ "$RC" -ne 0 ] && ! printf '%s' "$OUT" | grep -qE '^(REMOVED|RECLAIMED)'; then
  pass "UNKNOWN-FLAG «--dryrun»: отказ (exit=$RC), боевой прогон не запущен"
else
  fail "UNKNOWN-FLAG «--dryrun»: exit=$RC — неизвестный аргумент молча стал РЕАЛЬНЫМ прогоном; \
пользователь набирал превью"
fi
rm -rf "$S"

# ─── КОМБИНАЦИЯ ФЛАГОВ: `--dry-run --reclaim` не теряет reclaim ──────────────────────────
RAN=$((RAN + 1))
S="$(mk_sandbox)" || setup_fail "песочница DRY-PLUS-RECLAIM"
OUT="$(run_gc "$S" --dry-run --reclaim 3)"; RC=$?
if [ "$RC" -eq 0 ] && printf '%s' "$OUT" | grep -qE '^WOULD-RECLAIM' \
   && alive "$S/wt-fresh" && alive "$S/wt-idle"; then
  pass "DRY-PLUS-RECLAIM: режим reclaim исполнен как превью, ничего не удалено"
else
  fail "DRY-PLUS-RECLAIM: exit=$RC, WOULD-RECLAIM в выводе=$(printf '%s' "$OUT" | grep -cE '^WOULD-RECLAIM') \
— порядок флагов не смеет молча отменять режим"
fi
rm -rf "$S"

# ─── ПЕРЕПОЛНЕНИЕ: валидация обязана совпадать со СВОИМ ПОТРЕБИТЕЛЕМ ─────────────────────
# R-058 Б-1rev2. Текстовая проверка «только цифры» пропускает любую строку цифр, а `[ -lt ]`
# парсит лишь до intmax — за границей `[` возвращает 2, и управление проваливается в rm -rf.
# Ровно диагноз первого круга, сдвинутый на одну границу правее.
RAN=$((RAN + 1))
S="$(mk_sandbox)" || setup_fail "песочница OVERFLOW"
OUT="$(run_gc "$S" --reclaim 99999999999999999999)"; RC=$?
if [ "$RC" -ne 0 ] && alive "$S/wt-fresh" && alive "$S/wt-idle"; then
  pass "OVERFLOW «20 цифр»: отказ (exit=$RC), кэш цел"
else
  fail "OVERFLOW «20 цифр»: exit=$RC, свежий=$(alive "$S/wt-fresh" && echo цел || echo СНЕСЁН) — \
цифровой порог за intmax роняет сравнение так же, как «3ч»"
fi
rm -rf "$S"

# Граница снимается ТОЧНО, а не «где-то там»: ровно intmax обязан РАБОТАТЬ, иначе фикс
# ужесточён мимо цели и мы получили бы отказ на легитимном входе.
RAN=$((RAN + 1))
S="$(mk_sandbox)" || setup_fail "песочница INTMAX"
OUT="$(run_gc "$S" --reclaim 9223372036854775807)"; RC=$?
if [ "$RC" -eq 0 ] && alive "$S/wt-fresh" && alive "$S/wt-idle"; then
  pass "INTMAX «9223372036854775807»: принят, оба дерева KEEP-CACHE (граница не съехала внутрь)"
else
  fail "INTMAX: exit=$RC — ровно intmax обязан парситься; отказ здесь значит, что валидация \
строже своего потребителя, то есть ошибка в другую сторону"
fi
rm -rf "$S"

# ─── КЛАСС, А НЕ ЭКЗЕМПЛЯР: второй барьер держит САМ, без валидации ──────────────────────
# Два круга подряд закрывались добавлением ещё одной проверки ВХОДА — и оба раза находился
# вход, который проверку обходит. Форм входа бесконечно много; поэтому предмет этого
# сценария не форма, а КОНСТРУКЦИЯ: удаление требует утвердительного «да» (`-ge` истинно),
# а не отсутствия «нет». Мутант с ВЫКЛЮЧЕННОЙ валидацией обязан всё равно не удалять на
# входе, который роняет сравнение. Это и есть проверка того, что барьеров ДВА и они
# независимы: если сценарий краснеет, значит валидация — единственный несущий страж,
# и любая будущая правка, ослабившая её, снова откроет rm -rf.
RAN=$((RAN + 1))
S="$(mk_sandbox)" || setup_fail "песочница CLASS-SECOND-BARRIER"
MUT="$S/gc-novalidate.sh"
sed 's/^validate_threshold() {/validate_threshold() { return 0;/' "$GC" >"$MUT"
if ! grep -q 'validate_threshold() { return 0;' "$MUT"; then
  setup_fail "CLASS-SECOND-BARRIER: мутация НЕ применилась — сценарий проверял бы \
неизменённый скрипт и был бы зелёным по построению (плацебо самого себя)"
else
  OUT="$(run_gc_with "$MUT" "$S" --reclaim 3ч)"; RC=$?
  if alive "$S/wt-fresh" && alive "$S/wt-idle" \
     && ! printf '%s' "$OUT" | grep -qE '^RECLAIMED'; then
    pass "CLASS-SECOND-BARRIER: при выключенной валидации негодный порог НЕ удаляет (exit=$RC) \
— несущих стража два, и они независимы"
  else
    fail "CLASS-SECOND-BARRIER: свежий=$(alive "$S/wt-fresh" && echo цел || echo СНЕСЁН), \
состаренный=$(alive "$S/wt-idle" && echo цел || echo СНЕСЁН). Валидация — ЕДИНСТВЕННЫЙ страж \
разрушительного пути: класс не закрыт, закрыты лишь перечисленные формы входа"
  fi
fi
rm -rf "$S"

# ─── число исполненного СЧИТАЕТСЯ, а не заявляется ───────────────────────────────────────
echo
if [ "$RAN" -ne "$EXPECT_SCENARIOS" ]; then
  echo "FAIL  исполнено ${RAN} сценариев при объявленных ${EXPECT_SCENARIOS} — часть не отработала"
  FAILED=$((FAILED + 1))
fi
if [ "$FAILED" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} нарушений из ${RAN} сценариев)"
  exit 1
fi
echo "VERDICT: PASS (${RAN}/${EXPECT_SCENARIOS} сценариев)"
