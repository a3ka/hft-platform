#!/usr/bin/env bash
# Acceptance-гейт M-60a — замок процессного слоя.
#
# Решение по КОДУ ВОЗВРАТА (gates.md §3). Агрегатор со счётчиком: печатаем все нарушения,
# exit 1 при FAIL>0 — первый красный шаг не должен скрывать остальные.
#
# ТРИ УРОКА ВСТРОЕНЫ ЗДЕСЬ, каждый стоил ложного вердикта:
#  1. Число исполненных проверок СЧИТАЕТСЯ, а не заявляется. `VERDICT: PASS (0/0)` —
#     зелёная строка, не исполнившая ничего; так прошли пять пустых прогонов подряд.
#  2. «Проба зелёная» ≠ «проба ловит». Шаг F2 гоняет батарею: эталон обязан быть зелёным,
#     каждая дырявая реализация — красной (состав — спека §4.1, число печатает сама батарея). Без этого F проверяет лишь, что проба не падает.
#  3. Подключённость к CI НЕ доказывается разбором текста. Предусловие 3 проверяется
#     ИСПОЛНЕНИЕМ guard'а с подставленным `result=failure` (`A-005` §3): четыре круга
#     ужесточали регулярку, и rev4-парсер всё равно объявил выполненным предусловие на
#     фикстуре, где `exit 1` отключён через `if: ${{ false }}`.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}" || exit 1

FAILED=0
pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }

BARRIER=scripts/check_docs_freeze.sh
PROBE=scripts/tests/red_docs_freeze.sh
CI=.github/workflows/ci.yml

echo "--- A: барьер существует, исполним, парсится, fail-closed на пустой базе ---"
if [ -f "${BARRIER}" ] && bash -n "${BARRIER}" 2>/dev/null; then
  pass "A ${BARRIER} на месте и парсится"
  # «База не установлена» обязана быть ОТКАЗОМ, а не пропуском: иначе force-push и
  # создание ветки обходят замок (блокер B1, C-006).
  if ( EVENT_NAME=push PUSH_BEFORE="" PR_BASE_SHA="" bash "${BARRIER}" >/dev/null 2>&1 ); then
    fail "A барьер ПРОПУСТИЛ пустую базу — не fail-closed"
  else
    pass "A пустая база: fail-closed"
  fi
else
  fail "A ${BARRIER} отсутствует или не парсится"
fi

echo "--- F: RED-проба замка (состав сверяет сама проба — с исполнением и со спекой) ---"
# Числа сценариев здесь НЕТ намеренно: литерал, живущий отдельно от предмета, врёт всегда
# (шапка 12 / фактических 13 / порог >=11 — `C-066`; «15 сценариев» при 17 — `A-005` §2).
# Проба несёт манифест и валит прогон при расхождении состава в любую сторону.
if bash "${PROBE}" >/tmp/m60a-probe.log 2>&1; then
  pass "F проба зелёная: $(grep -oE 'VERDICT: PASS \([0-9]+/[0-9]+\)' /tmp/m60a-probe.log | head -1)"
else
  fail "F проба КРАСНАЯ — $(grep -E '^(VERDICT|SETUP)' /tmp/m60a-probe.log | head -1)"
  grep -E '^(FAIL|SETUP)' /tmp/m60a-probe.log | head -8 | sed 's/^/      ↳ /'
fi

echo "--- F2: АНТИ-ПЛАЦЕБО — батарея: эталон + дырявые реализации (спека §4.1) ---"
# «Проба не падает» и «проба ловит обход» — разные утверждения. Второе проверяется только
# так: против честной реализации проба обязана зеленеть, против каждой дырявой — краснеть.
# Часть мутантов — те, против которых rev4 была ЗЕЛЁНОЙ (`A-005` §10); `quotedpath` — тот,
# против которого был зелёным САМ ЭТАЛОН до rev7 (адверсарный прогон).
if bash "${PROBE}" --battery >/tmp/m60a-battery.log 2>&1; then
  pass "F2 $(grep -oE 'BATTERY: PASS \([0-9]+/[0-9]+\)' /tmp/m60a-battery.log | head -1)"
else
  fail "F2 батарея КРАСНАЯ — $(grep -E '^BATTERY' /tmp/m60a-battery.log | head -1)"
  grep -E '^(FAIL|SETUP)' /tmp/m60a-battery.log | head -8 | sed 's/^/      ↳ /'
fi

echo "--- S: САМОРЕФЕРЕНЦИЯ — M-60a обязан пройти собственный замок ---"
# Механизм, не выполняющий сам себя, не готов (C-062 §d). Диапазон — вся ветка над main.
if [ -f "${BARRIER}" ]; then
  BASE=$(git merge-base origin/main HEAD 2>/dev/null || echo "")
  if [ -z "${BASE}" ]; then
    fail "S не удалось установить базу (origin/main недоступен) — самореференцию не проверить"
  elif EVENT_NAME=push PUSH_BEFORE="${BASE}" PR_BASE_SHA="${BASE}" bash "${BARRIER}" >/dev/null 2>&1; then
    pass "S собственный диф проходит замок (токен founder'а на месте)"
  else
    fail "S собственный диф НЕ проходит замок — механизм не выполняет себя"
  fi
else
  fail "S замка нет — самореференцию проверять нечем"
fi

echo "--- W: ПРЕДУСЛОВИЯ блокировки; предусловие 3 — ИСПОЛНЕНИЕМ (§3ter «б» + §6.1) ---"
# Гейт НЕ доказывает, что красное блокирует merge: этого факта в репозитории нет, а на
# ЭТОМ репозитории его не может быть вовсе — branch protection возвращает 403 (private +
# free plan, замер `A-005` §8.5). Проверяются три предусловия со стороны репозитория; их
# отсутствие означает, что блокировки точно нет.
if [ ! -f "${CI}" ]; then
  fail "W ${CI} отсутствует"
else
  python3 - "$CI" <<'PYW' || FAILED=$((FAILED + 1))
import os, re, subprocess, sys, tempfile

try:
    import yaml
except Exception:
    print("FAIL  W PyYAML недоступен. Предусловие 3 проверяется ИСПОЛНЕНИЕМ; суррогатный")
    print("      ↳ разбор регуляркой уже дал ложный PASS на фикстуре C-067 — не подменяем.")
    print("      ↳ поставь: apt install python3-yaml  |  pip install pyyaml")
    sys.exit(1)

LOCK  = ('scripts/check_docs_freeze.sh', 'scripts/tests/red_docs_freeze.sh')
GUARD = 'status-check'
EXPR_NEEDS = re.compile(r'\$\{\{\s*needs\.([A-Za-z0-9_-]+)\.result\s*\}\}')
EXPR_ANY   = re.compile(r'\$\{\{.*?\}\}', re.S)


def jobs_of(text):
    try:
        doc = yaml.safe_load(text)
    except Exception:
        return {}
    if not isinstance(doc, dict):
        return {}
    j = doc.get('jobs')
    return j if isinstance(j, dict) else {}


def run_steps(job):
    """Шаги с РЕАЛЬНЫМ телом `run:`. Имя шага запуском не является (`C-065`, блокер 2:
       `- name: scripts/check_x.sh` удовлетворял и грепу, и первой редакции разбора)."""
    return [s for s in (job.get('steps') or [])
            if isinstance(s, dict) and isinstance(s.get('run'), str)]


def owners(jobs, script):
    return sorted(n for n, j in jobs.items() if any(script in s['run'] for s in run_steps(j)))


def needs_of(job):
    """ТОЛЬКО ключ `needs:`. Подстрочный поиск слова «needs» по телу джоба засчитывал
       упоминание `needs.<job>.result` внутри самого guard'а как объявление зависимости —
       джоб мог не быть в `needs` вовсе и стартовать параллельно проверяемому."""
    n = job.get('needs')
    if n is None:
        return []
    if isinstance(n, str):
        return [n]
    return [str(x) for x in n]


def evaluable(job):
    """Шаги, которые можно исполнить ДОСТОВЕРНО. Условный шаг исполнить нельзя (выражения
       GitHub мы не вычисляем) ⇒ консервативно считается неисполняющимся: ровно эта
       осторожность закрывает спуф `if: ${{ false }}` + `exit 1` (`C-067`)."""
    out = []
    for s in run_steps(job):
        if 'if' in s:
            continue
        sh = s.get('shell')
        if sh is not None and str(sh).strip() != 'bash':
            continue
        out.append(s['run'])
    return out


def substitute(text, failing):
    return EXPR_NEEDS.sub(lambda m: 'failure' if m.group(1) == failing else 'success', text)


def job_exit(bodies, failing, workdir):
    """Исполняет шаги подряд, как GitHub: первый ненулевой код валит джоб.
       `bash -e` — форма, которой GitHub зовёт `run:` по умолчанию (`shell:` в этом
       workflow не задан ни разу). Рабочий каталог — временный: guard не смеет
       ничего трогать в репозитории."""
    for i, body in enumerate(bodies):
        src = substitute(body, failing)
        if EXPR_ANY.search(src):
            continue  # осталось невычислимое выражение — шаг не исполняем (fail-closed)
        p = os.path.join(workdir, 'step%d.sh' % i)
        with open(p, 'w', encoding='utf-8') as f:
            f.write(src)
        rc = subprocess.run(['bash', '-e', p], cwd=workdir,
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode
        if rc != 0:
            return rc
    return 0


def n_executable(bodies, failing):
    return sum(0 if EXPR_ANY.search(substitute(b, failing)) else 1 for b in bodies)


def preconditions(text, workdir):
    """→ (p1, p2, p3, подробности). Каждое предусловие — независимое утверждение."""
    jobs = jobs_of(text)
    per_script = dict((s, owners(jobs, s)) for s in LOCK)
    own = sorted(set(x for v in per_script.values() for x in v))
    p1 = all(per_script[s] for s in LOCK)
    guard = jobs.get(GUARD)
    if guard is None or not own:
        why = 'джоба %s нет' % GUARD if guard is None else 'ни один джоб не исполняет скрипты замка'
        return p1, False, False, per_script, own, why
    needs = needs_of(guard)
    p2 = all(j in needs for j in own)
    bodies = evaluable(guard)
    p3, why = True, []
    for j in own:
        if n_executable(bodies, j) == 0:
            p3 = False
            why.append('%s: у %s нет ни одного безусловного исполнимого run-шага' % (j, GUARD))
            continue
        rc_fail = job_exit(bodies, j, workdir)
        rc_ok = job_exit(bodies, None, workdir)
        if rc_fail == 0:
            p3 = False
            why.append('%s: при result=failure guard вышел НУЛЁМ — красное не блокирует' % j)
        elif rc_ok != 0:
            p3 = False
            why.append('%s: при ВСЕХ success guard вышел %d — блокирует всё подряд, '
                       'то есть не блокирует ничего осмысленного' % (j, rc_ok))
    return p1, p2, p3, per_script, own, '; '.join(why)


# ─── САМОПРОВЕРКА ОРАКУЛА: шесть фикстур, каждая — контроль ОДНОГО утверждения ───────
FX_GOOD = """
name: fx
on: [push]
jobs:
  docs-freeze:
    runs-on: ubuntu-latest
    steps:
      - run: bash scripts/check_docs_freeze.sh
      - run: bash scripts/tests/red_docs_freeze.sh
  status-check:
    runs-on: ubuntu-latest
    needs: [docs-freeze]
    if: always()
    steps:
      - run: |
          if [[ "${{ needs.docs-freeze.result }}" != "success" ]]; then
            echo "one or more checks failed"; exit 1
          fi
          echo "all checks passed"
"""
FX_ECHO = FX_GOOD.replace("""      - run: |
          if [[ "${{ needs.docs-freeze.result }}" != "success" ]]; then
            echo "one or more checks failed"; exit 1
          fi
          echo "all checks passed"
""", """      - run: echo "docs-freeze=${{ needs.docs-freeze.result }}"
""")
FX_DISABLED = FX_GOOD.replace("""      - run: |
          if [[ "${{ needs.docs-freeze.result }}" != "success" ]]; then
            echo "one or more checks failed"; exit 1
          fi
          echo "all checks passed"
""", """      - if: ${{ false }}
        run: |
          if [[ "${{ needs.docs-freeze.result }}" != "success" ]]; then
            echo "one or more checks failed"; exit 1
          fi
      - run: echo "docs-freeze=${{ needs.docs-freeze.result }}"
""")
FX_NOTNEEDS = FX_GOOD.replace("    needs: [docs-freeze]", "    needs: [build-test]") + """
  build-test:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test --all
"""
FX_NAMEONLY = FX_GOOD.replace("""      - run: bash scripts/check_docs_freeze.sh
      - run: bash scripts/tests/red_docs_freeze.sh
""", """      - name: scripts/check_docs_freeze.sh
        run: echo подключено
      - name: scripts/tests/red_docs_freeze.sh
        run: echo подключено
""")
FX_CONSTFAIL = FX_GOOD.replace("""      - run: |
          if [[ "${{ needs.docs-freeze.result }}" != "success" ]]; then
            echo "one or more checks failed"; exit 1
          fi
          echo "all checks passed"
""", """      - run: exit 1
""")

SELFTEST = [
    ('корректный',                        FX_GOOD,      (True,  True,  True)),
    ('C-066 echo-only',                   FX_ECHO,      (True,  True,  False)),
    ('C-067 exit 1 под if: false',        FX_DISABLED,  (True,  True,  False)),
    ('джоб НЕ в needs',                   FX_NOTNEEDS,  (True,  False, True)),
    ('C-065 скрипт только в name:',       FX_NAMEONLY,  (False, False, False)),
    ('guard = безусловный exit 1',        FX_CONSTFAIL, (True,  True,  False)),
]

ok = True
with tempfile.TemporaryDirectory() as wd:
    bad = []
    for label, text, expected in SELFTEST:
        got = preconditions(text, wd)[:3]
        if got != expected:
            bad.append('%s: ожидалось %s, получено %s' % (label, expected, got))
    if bad:
        print("FAIL  W САМОПРОВЕРКА ОРАКУЛА не прошла — оракул W сам сломан, вердикту верить нельзя")
        for b in bad:
            print("      ↳ " + b)
        ok = False
    else:
        print("PASS  W самопроверка оракула: %d фикстур классифицированы верно "
              "(корректная · echo-only · exit под if:false · не в needs · только name: · "
              "безусловный exit 1)" % len(SELFTEST))

    # ─── Вердикт по РЕАЛЬНОМУ workflow ───────────────────────────────────────────────
    with open(sys.argv[1], encoding='utf-8') as f:
        real = f.read()
    p1, p2, p3, per_script, own, why = preconditions(real, wd)

    for s in LOCK:
        if per_script.get(s):
            print("PASS  W предусловие 1: %s ИСПОЛНЯЕТСЯ джобом(ами): %s" % (s, ', '.join(per_script[s])))
        else:
            print("FAIL  W предусловие 1: %s не встречается ни в одном теле `run:` — "
                  "построен, но не подключён" % s)
    if not p1:
        ok = False
    if own:
        if p2:
            print("PASS  W предусловие 2: %s в ключе %s.needs" % (', '.join(own), GUARD))
        else:
            print("FAIL  W предусловие 2: %s отсутствует в ключе %s.needs" % (', '.join(own), GUARD))
            ok = False
        if p3:
            print("PASS  W предусловие 3 (ИСПОЛНЕНИЕМ): guard падает при result=failure "
                  "и выходит нулём при всех success")
        else:
            print("FAIL  W предусловие 3 (ИСПОЛНЕНИЕМ): %s" % (why or 'guard не блокирует'))
            ok = False
    else:
        print("FAIL  W предусловия 2-3 не проверить: %s" % why)
        ok = False

    print("      ↳ NB: даже все три PASS не означают, что merge заблокирован — блокировку "
          "включает branch protection, на этом репо недоступный (403, private+free, A-005 §8.5).")
sys.exit(0 if ok else 1)
PYW
fi

echo "--- P: РЕГРЕСС — соседний барьер артефактов цел ---"
if bash scripts/tests/red_protected_artifacts.sh >/tmp/m60a-prot.log 2>&1; then
  pass "P $(grep -oE 'VERDICT: PASS \([0-9]+/[0-9]+\)' /tmp/m60a-prot.log | head -1)"
else
  fail "P барьер артефактов сломан этим milestone'ом — цена замка уплачена соседним инвариантом"
fi

echo "--- T: паритет с CI-job fmt+clippy+test (gates.md §3) ---"
cargo fmt --all -- --check >/dev/null 2>&1 && pass "T fmt" || fail "T fmt --check"
cargo clippy --workspace --all-targets --all-features -- -D warnings >/tmp/m60a-clippy.log 2>&1 \
  && pass "T clippy" || { fail "T clippy"; tail -5 /tmp/m60a-clippy.log | sed 's/^/      ↳ /'; }
cargo test --all >/tmp/m60a-test.log 2>&1 \
  && pass "T cargo test --all" || { fail "T cargo test --all"; grep -E '^test .* FAILED' /tmp/m60a-test.log | head -5 | sed 's/^/      ↳ /'; }

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "VERDICT: FAIL (${FAILED} нарушений)"
  exit 1
fi
echo "VERDICT: PASS"
