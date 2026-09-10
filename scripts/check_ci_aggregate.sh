#!/usr/bin/env bash
# Барьер: КАЖДЫЙ джоб, от которого агрегат зависит, обязан ВЛИЯТЬ на его исход.
#
# ── ЗАМЕР, ИЗ-ЗА КОТОРОГО БАРЬЕР ЗАВЕДЁН (2026-09-09) ────────────────────────────────
# Джоб `status-check` («All checks passed») — единственный обязательный чек защиты ветки:
# именно его зелёный цвет пускает merge в `main`. Он объявлен `if: always()` и решает по
# ЯВНОМУ перечню `needs.<job>.result`. Замер показал расхождение двух списков:
#
#   в needs: 17 · проверяется в условии: 15 · ИГНОРИРУЕТСЯ: secret-material, roadmap-sync
#
# Следствие не косметическое. `secret-material` — барьер «ключи не живут в репозитории».
# Он был в зависимостях, поэтому агрегат его ДОЖИДАЛСЯ, и на глаз всё выглядело исправно.
# Но его результат не читался: упади он — агрегат всё равно печатает «All checks passed»,
# защита ветки видит зелёный чек и пускает merge. То же с `roadmap-sync`.
# Оба построены как БЛОКИРУЮЩИЕ и оба были ДЕКОРАТИВНЫМИ.
#
# ── ПОЧЕМУ ЭТО КЛАСС, А НЕ ДВА СЛУЧАЯ ───────────────────────────────────────────────
# Добавление джоба требует правки ДВУХ мест: `needs` и условия. Забыть второе легко, и
# ошибка МОЛЧАЛИВА — CI зелен, барьер «есть», merge проходит. Ровно `gates.md` §11:
# «правило, которое механизировать нечем, объявляет себя COGNITIVE-ONLY вместо того, чтобы
# изображать гейт». Здесь гейт изображался сам собой — и заметил это не человек, а
# посторонний замер при добавлении соседнего джоба.
#
# ── ПРЕДЕЛ НАЗВАН ───────────────────────────────────────────────────────────────────
# Барьер сверяет ДВА СПИСКА в одном файле. Он НЕ проверяет: что джоб делает осмысленную
# работу; что он вообще запускается на нужных событиях; что его скрипт не всегда-зелёный.
# Это остаётся за пробами самих барьеров и за кругом гейта.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CI="${CI_WORKFLOW:-${ROOT}/.github/workflows/ci.yml}"
AGG="${CI_AGGREGATE_JOB:-status-check}"
[ -f "${CI}" ] || { echo "SETUP НЕ СОСТОЯЛСЯ: нет ${CI}" >&2; exit 2; }

python3 - "${CI}" "${AGG}" <<'PY'
import re, sys
path, agg = sys.argv[1], sys.argv[2]
s = open(path, encoding='utf-8').read()
m = re.search(rf'^  {re.escape(agg)}:\n(.*?)(?=^  \S|\Z)', s, re.M | re.S)
if not m:
    print(f"SETUP НЕ СОСТОЯЛСЯ: джоб {agg} не найден в {path}"); sys.exit(2)
blk = m.group(1)
mn = re.search(r'needs:\s*\[([^\]]*)\]', blk, re.S)
if not mn:
    print(f"SETUP НЕ СОСТОЯЛСЯ: у {agg} нет списка needs"); sys.exit(2)
needs = [x.strip() for x in mn.group(1).replace('\n', ' ').split(',') if x.strip()]
checked = set(re.findall(r"needs\.([a-zA-Z0-9_-]+)\.result", blk))
if not needs:
    print(f"SETUP НЕ СОСТОЯЛСЯ: список needs пуст"); sys.exit(2)

ignored = [n for n in needs if n not in checked]
phantom = sorted(checked - set(needs))
fail = 0
if ignored:
    print(f"FAIL  {len(ignored)} джоб(ов) в needs НЕ ВЛИЯЮТ на исход агрегата — их падение")
    print( "      игнорируется, агрегат печатает «All checks passed», защита ветки пускает merge:")
    for n in ignored:
        print(f"        · {n}")
    print( "      Лечится добавлением в условие: || \"${{ needs.<job>.result }}\" != \"success\"")
    fail += 1
else:
    print(f"PASS  все {len(needs)} джоб(ов) из needs влияют на исход агрегата")
if phantom:
    print(f"FAIL  {len(phantom)} джоб(ов) проверяются в условии, но ОТСУТСТВУЮТ в needs —")
    print( "      агрегат не дожидается их и читает пустой результат:")
    for n in phantom:
        print(f"        · {n}")
    fail += 1
else:
    print("PASS  в условии нет джобов, отсутствующих в needs")
print()
if fail:
    print(f"VERDICT: FAIL ({fail})"); sys.exit(1)
print("VERDICT: PASS — списки зависимостей и проверяемых результатов совпадают")
PY
