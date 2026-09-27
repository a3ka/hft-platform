#!/usr/bin/env bash
# red_ci_aggregate.sh — ПРОБА агрегата «All checks passed» (`.github/workflows/ci.yml`, джоб
# `status-check`). Защита `main` требует ЕДИНСТВЕННЫЙ чек — этот; провал, которого он не видит,
# merge не держит.
#
# ЗАМЕР, ИЗ-ЗА КОТОРОГО ПРОБА ЗАВЕДЕНА (2026-09-26). Условие агрегата — РУЧНОЙ список
# `needs.<job>.result`; в нём было 16 джобов при 18 в `needs`: `secret-material` и
# `roadmap-sync` (обе с 2026-09-07) выпали, и при `if: always()` их провал агрегат не видел.
#
# ПОЧЕМУ НЕ «УСЛОВИЕ ИЗ toJSON(needs)». Эта форма была первой редакцией и отвергнута `C-257`
# R2: на рукописную форму условия опираются два действующих барьера — `deploy_catchup.py
# check-aggregate` (исполняет `steps[0]` агрегата на модели результатов) и `red_review_fa.sh`
# W8 — и обе краснели. Сохранена форма, лечится КЛАСС: проба ИСПОЛНЯЕТ условие для КАЖДОГО
# джоба из `needs` поодиночке. Джоб, выпавший из условия, называется поимённо.
#
# ЧТО ИСПОЛНЯЕТСЯ. Не копия логики, а ТОТ ЖЕ шаг из `ci.yml`, выбранный СТРУКТУРНО: шаг
# `status-check`, чей `run` ссылается на `needs.<job>.result`. Кандидатов ровно один — иначе
# SETUP-FAIL (`C-257` R1: проба не вправе исполнять «первый похожий» блок). Форма вызова —
# как у раннера: `bash --noprofile --norc -eo pipefail -c`.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CI="${CI_AGG_YML:-$ROOT/.github/workflows/ci.yml}"

python3 - "$CI" <<'PY'
import copy, re, subprocess, sys
import yaml

N = 0; BAD = 0
def ok(m):
    global N; N += 1; print(f"ok    {m}")
def bad(m):
    global N, BAD; N += 1; BAD += 1; print(f"FAIL  {m}")

# НАБЛЮДАТЕЛИ — merge не держат по замыслу (`docs/workflow/reading-map.md` §2, Ярус S).
OBSERVERS = {"branch-health"}
GATE = "status-check"
REF = re.compile(r"\$\{\{\s*needs\.([A-Za-z0-9_-]+)\.result\s*\}\}")

def analyse(wf):
    """Вернуть список нарушений (пустой = агрегат верен). SETUP-ошибки — тоже нарушения."""
    out = []
    jobs = wf.get("jobs") or {}
    gate = jobs.get(GATE)
    if not isinstance(gate, dict):
        return [f"SETUP: джоба {GATE} нет"]
    needs = gate.get("needs") or []
    if isinstance(needs, str): needs = [needs]
    if len(needs) < 5:
        return [f"SETUP: в {GATE}.needs {len(needs)} джобов — ci.yml разобран не тот"]
    if gate.get("if") != "always()":
        out.append(f"{GATE}.if = {gate.get('if')!r}, не always(): провал зависимого джоба даст skipped, не failure")
    cands = [s for s in gate.get("steps") or []
             if isinstance(s, dict) and isinstance(s.get("run"), str) and REF.search(s["run"])]
    if len(cands) != 1:
        return out + [f"SETUP: шагов-условий в {GATE} {len(cands)}, нужен ровно один — какой исполнять, неоднозначно"]
    step = cands[0]
    if step.get("continue-on-error") or str(step.get("if", "")).strip() in ("false", "${{ false }}"):
        out.append("шаг-условие обезврежен (continue-on-error / if: false)")
    body = step["run"]
    def run(results):
        b = REF.sub(lambda m: results.get(m.group(1), "success"), body)
        b = re.sub(r"\$\{\{[^}]*\}\}", "", b)
        p = subprocess.run(["bash", "--noprofile", "--norc", "-eo", "pipefail", "-c", b],
                           capture_output=True, text=True, timeout=30)
        return p.returncode
    if run({}) != 0:
        out.append("все success, а условие падает — «падать всегда» прошло бы проверку ниже вакуумно")
    blind = [j for j in needs if run({j: "failure"}) == 0]
    if blind:
        out.append(f"провал джоба НЕ роняет агрегат — выпали из условия: {', '.join(sorted(blind))}")
    for r in ("cancelled", "skipped"):
        if run({needs[0]: r}) == 0:
            out.append(f"исход {r} у {needs[0]} принят за успех")
    missing = sorted(set(jobs) - set(needs) - {GATE} - OBSERVERS)
    if missing:
        out.append(f"джобы вне needs агрегата (их провал merge не держит): {', '.join(missing)}")
    return out

wf = yaml.safe_load(open(sys.argv[1], encoding="utf-8"))

# (1) ПРЕДМЕТ: реальный ci.yml
v = analyse(wf)
ok("реальный агрегат: каждый джоб needs роняет условие, все success проходят, cancelled/skipped — отказ, состав полон") if not v else bad("реальный агрегат: " + " | ".join(v))

# (2..) МЕТА-МУТАНТЫ: анализ обязан КРАСНЕТЬ. Без них зелёный (1) ничего не доказывает.
# Предмет не разобрался (SETUP) — мутанты над ним бессмысленны: отказ уже засчитан в (1).
BASE_SETUP = any(x.startswith("SETUP") for x in v)
def mutant(name, fn, expect_substr):
    if BASE_SETUP:
        return
    m = copy.deepcopy(wf); fn(m)
    if m == wf:
        bad(f"SETUP мутанта «{name}»: мутация не применилась"); return
    v = analyse(m)
    hit = any(expect_substr in x for x in v)
    ok(f"мутант «{name}» пойман") if hit else bad(f"мутант «{name}» НЕ пойман: {v}")

def cond_step(m):
    return [s for s in m["jobs"][GATE]["steps"] if isinstance(s.get("run"), str) and REF.search(s["run"])][0]
def drop_from_cond(job):
    def f(m):
        st = cond_step(m)
        st["run"] = re.sub(r'\s*\|\|\s*"\$\{\{\s*needs\.' + re.escape(job) + r'\.result\s*\}\}"\s*!=\s*"success"', "", st["run"])
    return f

mutant("secret-material выпал из условия", drop_from_cond("secret-material"), "secret-material")
mutant("roadmap-sync выпал из условия", drop_from_cond("roadmap-sync"), "roadmap-sync")
mutant("джоб выпал из needs", lambda m: m["jobs"][GATE].__setitem__("needs", [n for n in m["jobs"][GATE]["needs"] if n != "contracts"]), "contracts")
mutant("условие «падать всегда»", lambda m: cond_step(m).__setitem__("run", cond_step(m)["run"].replace('echo "All checks passed"', 'exit 1')), "все success")
mutant("снят if: always()", lambda m: m["jobs"][GATE].pop("if"), "always()")
# `C-257` R1: здоровая копия условия ПЕРЕД настоящим + сломанное настоящее. Проба не вправе
# зеленеть, исполнив первую копию: два кандидата = SETUP.
def decoy(m):
    real = cond_step(m); good = copy.deepcopy(real)
    real["run"] = 'echo "All checks passed"'  # сломано: не смотрит ни на что
    real["run"] = real["run"] + "  # ${{ needs.build-test.result }}"
    m["jobs"][GATE]["steps"].insert(0, good)
mutant("здоровая копия-приманка + сломанное настоящее", decoy, "SETUP")
mutant("шаг-условие continue-on-error", lambda m: cond_step(m).__setitem__("continue-on-error", True), "обезврежен")

print()
if BAD == 0:
    print(f"VERDICT: PASS — сценариев: {N}, расхождений: 0"); sys.exit(0)
print(f"VERDICT: FAIL — сценариев: {N}, расхождений: {BAD}"); sys.exit(1)
PY
