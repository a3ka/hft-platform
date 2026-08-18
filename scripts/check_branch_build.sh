#!/usr/bin/env bash
# Барьер: ВЕТКА СОБИРАЕТСЯ — и собирается ТЕМ ЖЕ составом, что `main`, ничего не задевая.
#
# Предмет — `.github/workflows/branch-build.yml` (в его шапке: зачем отдельный workflow и
# почему предписанная правка `ci.yml` на `push.branches: ['**']` нерабочая).
#
# ЗАЧЕМ БАРЬЕР НУЖЕН САМОМУ МЕХАНИЗМУ. `branch-build.yml` — сборщик, а не наблюдатель: о
# собственном исчезновении он не говорит ничего. Удали файл — на `main` не покраснеет ничто,
# тропа снова станет нехоженой, и узнается это через круги гейта, как в августе. Ровно класс
# «наблюдает сбой, не наблюдает ОТСУТСТВИЕ» (`testing.md`, целостность гейта, свойство 4).
# Поэтому барьер заведён в `ci.yml` (job `branch-build-parity`) и входит в агрегат
# `All checks passed` — его красное блокирует merge. Проверку СОБСТВЕННОЙ проводки он несёт
# сам (B7): барьер, не наблюдающий своего отключения, — та же дыра одним уровнем выше
# (приём взят у `scripts/deploy_catchup.py`, там это A6).
#
# ДЕВЯТЬ ИНВАРИАНТОВ, каждый по РАЗБОРУ YAML, не грепом (греп зелен и против
# закомментированной строки, и против имени в соседнем `echo`):
#
#   B1  файл существует и разбирается как YAML                      ← наблюдение ОТСУТСТВИЯ
#   B2  триггер: `on.push.branches-ignore` содержит `main` и не глушит всё остальное
#   B3  ПАРИТЕТ состава с job `build-test` из `ci.yml` — в ОБЕ стороны
#   B4  джоб не обезврежен — на уровне ДЖОБА и на уровне ШАГА
#   B5  `ci.yml` не «починили» отклонённой правкой (`on.push.branches` остаётся `[main]`)
#   B6  `deploy.yml` не задет: push жёстко `[main]`, наш файл вне его `paths`, имя не в
#       `workflow_run`
#   B7  барьер РЕАЛЬНО заведён в `ci.yml`, стоит в `status-check.needs` и участвует в
#       fail-closed условии агрегата
#   B8  `concurrency`: группа есть, привязана к `github.ref` и не совпадает с чужой
#   B9  имена джобов не подделывают required-контекст; прав на запись не запрошено
#
# ПРЕДЕЛЫ, НАЗВАННЫЕ ЯВНО:
#   · Барьер судит ТЕКСТ конфигурации, а не факт прогона. Доказать, что GitHub действительно
#     запустил сборку, он не может: это предъявляется глазами — `gh run list --branch <ветка>`
#     (ярус S `docs/workflow/reading-map.md`). Барьер удерживает конфигурацию от гниения, и
#     это всё, что он обещает.
#   · Взаимоисключение `branches`/`branches-ignore` и семантика `paths` воспроизведены по
#     документации GitHub, а не проверены у GitHub.
#   · Сопоставление `paths` — упрощённое (`**` → `*` через `fnmatch`), достаточное для
#     вопроса «попадает ли НАШ файл под фильтр деплоя», но не полный движок GitHub.
#
# ЧТО ЗДЕСЬ ИСПРАВЛЕНО ПОСЛЕ ЗАМЕРА (первая редакция несла ложный «честно названный предел»).
# Она утверждала: «шаговый уровень обезвреживания уже ловится паритетом B3 — любая такая
# правка меняет строку команды». Для `|| true` это верно (проверено: B3 краснеет), а для
# КЛЮЧЕЙ `continue-on-error:`/`if:` на шаге — нет: они строку `run:` не трогают, и барьер
# давал PASS. Ложный предел хуже отсутствующей проверки: он объявляет дыру закрытой.
#
# ПРОД-ФОРМА — БЕЗ АРГУМЕНТОВ, из корня репозитория; именно так его зовёт `ci.yml`.
# `BRANCH_BUILD_ROOT` — ручка ПРОБЫ (`scripts/tests/red_branch_build.sh`) для синтетических
# фикстур; прод-путь её не задаёт.
#
# Прогон: bash scripts/check_branch_build.sh

set -uo pipefail

ROOT="${BRANCH_BUILD_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"

python3 - "${ROOT}" <<'PY'
import fnmatch
import os
import re
import sys

ROOT = sys.argv[1] if len(sys.argv) > 1 else "."
WF = os.path.join(ROOT, ".github", "workflows")
BB_REL = ".github/workflows/branch-build.yml"
BB = os.path.join(ROOT, BB_REL)
CI = os.path.join(WF, "ci.yml")
DP = os.path.join(WF, "deploy.yml")

REF_JOB = "build-test"          # джоб-эталон в ci.yml и его копия в branch-build.yml
AGG_GATE = "status-check"       # джоб-агрегат ci.yml, производящий required-контекст
WIRE_JOB = "branch-build-parity"  # джоб ci.yml, который обязан звать ЭТОТ барьер
BARRIER = "bash scripts/check_branch_build.sh"
PROBE = "bash scripts/tests/red_branch_build.sh"

FAILED = 0


def bad(code, msg):
    global FAILED
    FAILED += 1
    print(f"FAIL  {code}: {msg}")


def ok(code, msg):
    print(f"ok    {code}: {msg}")


# PyYAML отсутствует ⇒ барьер НЕ пропускает, а падает: «разобрать нечем» не значит
# «проверять нечего» — тот же fail-closed, что у баз события в `ci.yml`.
try:
    import yaml
except ImportError as exc:  # pragma: no cover — окружение без PyYAML
    print(f"FAIL  SETUP: PyYAML недоступен ({exc}) — разобрать workflow нечем, барьер fail-closed")
    sys.exit(1)


def load(path):
    if not os.path.isfile(path):
        return None, f"файла {os.path.relpath(path, ROOT)} НЕТ"
    try:
        with open(path, encoding="utf-8") as fh:
            data = yaml.safe_load(fh)
    except Exception as exc:
        return None, f"{os.path.relpath(path, ROOT)} не разбирается как YAML: {exc}"
    if not isinstance(data, dict):
        return None, f"{os.path.relpath(path, ROOT)}: ожидался маппинг, получено {type(data).__name__}"
    return data, None


def triggers(wf, label):
    """Секция `on:` — с ЯВНЫМ разбором коллизии ключей.

    Ловушка YAML 1.1: голый `on` — БУЛЕВ ключ, `yaml.safe_load` отдаёт его как `True`;
    закавыченный `"on"` даёт строку. Барьер, берущий первый попавшийся, при обоих ключах
    молча читает НЕ ТОТ блок: цвет может оказаться верным, а диагноз ложным — и при
    обратном порядке ключей это даёт ложное ЗЕЛЁНОЕ. Поэтому коллизия — отдельный отказ.
    """
    present = [k for k in ("on", True) if k in wf]
    if len(present) > 1:
        bad("B2", f"{label}: ключ `on` задан ДВАЖДЫ (строковый `\"on\"` и булев `on` YAML 1.1) — "
                  f"какой блок действует, определить нельзя")
        return None
    if not present:
        return None
    node = wf[present[0]]
    return node if isinstance(node, dict) else None


def job_of(wf, name):
    jobs = wf.get("jobs")
    if not isinstance(jobs, dict):
        return None
    job = jobs.get(name)
    return job if isinstance(job, dict) else None


def as_list(node):
    if node is None:
        return []
    return [str(x) for x in node] if isinstance(node, list) else [str(node)]


def steps_of(job):
    return [st for st in (job.get("steps") or []) if isinstance(st, dict)]


def norm_steps(job):
    """Нормализованная последовательность шагов — то, что раннер РЕАЛЬНО исполняет.

    Имена шагов отбрасываются (косметика); `uses` сверяется вместе с `with` (без
    `components: rustfmt, clippy` fmt и clippy упали бы по ЧУЖОЙ причине); `run` режется на
    строки с вычищенными комментариями и схлопнутыми пробелами. Сравниваются
    ПОСЛЕДОВАТЕЛЬНОСТИ, а не множества: порядок шагов — часть состава.
    """
    out = []
    for st in steps_of(job):
        if "uses" in st:
            with_ = st.get("with")
            items = (
                tuple(sorted((str(k), str(v).strip()) for k, v in with_.items()))
                if isinstance(with_, dict) else ()
            )
            out.append(("uses", str(st["uses"]).strip(), items))
        elif isinstance(st.get("run"), str):
            cmds = []
            for line in st["run"].splitlines():
                body = line.split("#", 1)[0].strip()
                if body:
                    cmds.append(re.sub(r"\s+", " ", body))
            out.append(("run", tuple(cmds)))
    return out


def show(step):
    if step[0] == "uses":
        return f"uses {step[1]} {dict(step[2]) if step[2] else ''}".strip()
    return "run " + " ; ".join(step[1])


def static_false(cond):
    return isinstance(cond, (bool, str)) and str(cond).strip().lower() in (
        "false", "${{ false }}", "0",
    )


def invokes(job, needle):
    """`needle` стоит В ПОЗИЦИИ КОМАНДЫ хотя бы в одном шаге.

    Отличие от подстроки принципиальное: `echo "check_branch_build.sh ок"` вызовом не
    является. Приём и его предел взяты у `scripts/deploy_catchup.py::_invokes`: полного
    разбора shell тут нет, `eval`/переменная в имени команды барьером не ловятся.
    """
    for st in steps_of(job):
        run = st.get("run")
        if not isinstance(run, str):
            continue
        for line in run.splitlines():
            body = line.split("#", 1)[0].strip()
            if not body:
                continue
            for seg in re.split(r"&&|\|\||;|\|", body):
                seg = seg.strip()
                if seg.startswith(("sudo ", "env ")):
                    seg = seg.split(None, 1)[1] if " " in seg else seg
                if seg.startswith(needle):
                    return True
    return False


def job_names(wf):
    """display-имена джобов: `name:` либо, при его отсутствии, идентификатор джоба —
    ровно то, что GitHub публикует как КОНТЕКСТ проверки."""
    jobs = wf.get("jobs") if isinstance(wf.get("jobs"), dict) else {}
    out = {}
    for jid, j in jobs.items():
        nm = j.get("name") if isinstance(j, dict) else None
        out[str(jid)] = str(nm).strip() if isinstance(nm, str) and nm.strip() else str(jid)
    return out


# ─── B2. Триггер ───────────────────────────────────────────────────────────────────────
def check_b2(trig):
    if not isinstance(trig, dict):
        bad("B2", "секции `on:` нет, она не маппинг или ключ задан дважды — workflow "
                  "не запускается ничем предсказуемым")
        return
    if "push" not in trig:
        bad("B2", f"в `on:` нет `push` (есть: {sorted(str(k) for k in trig)}) — сборка ветки не "
                  f"рождается событием, значит требует воли автора; ровно это и лечится")
        return
    push = trig["push"] if isinstance(trig["push"], dict) else {}
    ignore = as_list(push.get("branches-ignore"))
    branches = as_list(push.get("branches"))
    if branches and ignore:
        bad("B2", f"одновременно `branches` ({branches}) и `branches-ignore` ({ignore}) — "
                  f"для одного события ключи взаимоисключающие, поведение не определено")
        return
    if branches:
        bad("B2", f"`on.push.branches` = {branches} вместо `branches-ignore` — это фильтр "
                  f"ВКЛЮЧЕНИЯ: на ветках сборки по-прежнему не будет")
        return
    if not ignore:
        bad("B2", "в `on.push` нет `branches-ignore` — опечатка в имени ключа "
                  "(`branches_ignore`) молча даёт триггер на ВСЕ ветки, включая `main`, "
                  "то есть дубль `ci.yml`")
        return
    # ПОРЯДОК ВАЖЕН: catch-all проверяется ДО членства `main`. Иначе `['**']` — которое
    # исключает и `main`, и всё остальное — диагностировалось бы как «не исключает main»:
    # цвет верный, диагноз ложный, читатель чинит не то. Тот же класс, что коллизия `on`.
    catchall = [p for p in ignore if p in ("**", "*", "**/**")]
    if catchall:
        bad("B2", f"`branches-ignore` = {ignore} содержит {catchall} — исключает ВСЁ; "
                  f"механизм мёртв при формально верном ключе")
        return
    if "main" not in ignore:
        bad("B2", f"`branches-ignore` = {ignore} не исключает `main` — на `main` сборка "
                  f"продублирует `ci.yml`, съедая раннеры и путая чтение прогонов")
        return
    ok("B2", f"`on.push.branches-ignore` = {ignore} — ветки собираются, `main` исключён")


# ─── B3. Паритет состава ───────────────────────────────────────────────────────────────
def check_b3(bb, ci):
    ours, ref = job_of(bb, REF_JOB), job_of(ci, REF_JOB)
    if ref is None:
        bad("B3", f"SETUP НЕ СОСТОЯЛСЯ: в `ci.yml` нет джоба `{REF_JOB}` — сверять паритет не "
                  f"с чем; молчаливое «паритет ок» здесь было бы плацебо самого барьера")
        return
    if ours is None:
        bad("B3", f"в `branch-build.yml` нет джоба `{REF_JOB}`")
        return
    a, b = norm_steps(ours), norm_steps(ref)
    if a == b:
        ok("B3", f"состав `{REF_JOB}` совпадает с `ci.yml` шаг-в-шаг ({len(a)} шагов)")
        return
    bad("B3", f"состав `{REF_JOB}` РАЗОШЁЛСЯ с `ci.yml` — «ветка собирается» значит не то же, "
              f"что «`main` собирается»")
    only_ci = [s for s in b if s not in a]
    only_bb = [s for s in a if s not in b]
    for s in only_ci:
        print(f"      нет в branch-build (есть в ci.yml): {show(s)}")
    for s in only_bb:
        print(f"      лишнее в branch-build (нет в ci.yml): {show(s)}")
    if not only_ci and not only_bb:
        print("      состав тот же, но ПОРЯДОК шагов иной")


# ─── B4. Обезвреживание — джоб И шаги ──────────────────────────────────────────────────
def check_b4(bb):
    job = job_of(bb, REF_JOB)
    if job is None:
        bad("B4", f"джоба `{REF_JOB}` нет — обезвреживать нечего, но и собирать нечем")
        return
    if job.get("continue-on-error") in (True, "true"):
        bad("B4", f"`{REF_JOB}.continue-on-error: true` — падение сборки не делает прогон "
                  f"красным; ветка «собирается» всегда")
        return
    if static_false(job.get("if")):
        bad("B4", f"`{REF_JOB}.if: {job.get('if')!r}` — джоб не исполняется ни при каком "
                  f"событии; файл есть, механизма нет")
        return
    sts = steps_of(job)
    if not sts:
        bad("B4", f"у джоба `{REF_JOB}` нет ни одного шага")
        return
    for i, st in enumerate(sts, 1):
        label = st.get("name") or st.get("uses") or f"шаг {i}"
        if st.get("continue-on-error") in (True, "true"):
            bad("B4", f"шаг «{label}» несёт `continue-on-error: true` — его падение не роняет "
                      f"сборку. Паритет B3 этого НЕ видит: ключ не меняет строку `run:`")
            return
        if static_false(st.get("if")):
            bad("B4", f"шаг «{label}» несёт `if: {st.get('if')!r}` — не исполняется никогда; "
                      f"паритет B3 этого не видит по той же причине")
            return
    ok("B4", f"джоб `{REF_JOB}` и все {len(sts)} шагов не обезврежены")


# ─── B5. Отклонённая правка `ci.yml` не просочилась ────────────────────────────────────
def check_b5(ci):
    trig = triggers(ci, "ci.yml")
    if not isinstance(trig, dict) or not isinstance(trig.get("push"), dict):
        bad("B5", "в `ci.yml` нет `on.push` — проводка изменена неожидаемым образом")
        return
    branches = as_list(trig["push"].get("branches"))
    if branches != ["main"]:
        bad("B5", f"`ci.yml` `on.push.branches` = {branches}, ожидалось ['main']. Это ровно то "
                  f"предписание, что отклонено ИСПОЛНЕНИЕМ: базы события становятся "
                  f"недостоверными (zero-SHA на первом push'е, force-push) и падают пять "
                  f"джобов, а два прогона на одном SHA ломают `gh pr checks --watch`")
        return
    if as_list(trig["push"].get("branches-ignore")):
        bad("B5", "в `ci.yml` `on.push` появился `branches-ignore` — рядом с `branches` они "
                  "взаимоисключающи, поведение не определено")
        return
    ok("B5", "`ci.yml` `on.push.branches` = ['main'] — отклонённая правка не просочилась")


# ─── B6. Прод не шелохнётся ────────────────────────────────────────────────────────────
def check_b6(dp, bb_name):
    trig = triggers(dp, "deploy.yml")
    if not isinstance(trig, dict):
        bad("B6", "в `deploy.yml` нет разбираемой секции `on:`")
        return
    push = trig.get("push") if isinstance(trig.get("push"), dict) else {}
    branches = as_list(push.get("branches"))
    if branches != ["main"]:
        bad("B6", f"`deploy.yml` `on.push.branches` = {branches}, ожидалось ['main'] — сборка "
                  f"ветки не имеет права дотягиваться до прода")
    # Наш файл не должен попадать под фильтр путей деплоя: иначе КАЖДЫЙ его коммит = редеплой,
    # а редеплой = рестарт recorder'а = ГЭП в forward-only записи (класс TD-086).
    hits = [p for p in as_list(push.get("paths"))
            if not p.startswith("!") and fnmatch.fnmatch(BB_REL, p.replace("**", "*"))]
    if hits:
        bad("B6", f"`deploy.yml` `on.push.paths` накрывает {BB_REL} шаблоном {hits} — каждый "
                  f"коммит сборщика ветки станет редеплоем прода (рестарт recorder'а = гэп "
                  f"forward-only записи, класс TD-086)")
    wr = trig.get("workflow_run") if isinstance(trig.get("workflow_run"), dict) else {}
    named = as_list(wr.get("workflows"))
    if bb_name in named:
        bad("B6", f"`deploy.yml` `workflow_run.workflows` = {named} содержит «{bb_name}» — "
                  f"зелёная сборка ЛЮБОЙ ветки начала бы дёргать сторожа добора и выкатывать "
                  f"прод; путь к данным сборкой ветки не открывается")
    if branches == ["main"] and not hits and bb_name not in named:
        ok("B6", f"`deploy.yml`: push=['main'], paths не накрывают предмет, workflow_run={named}")


# ─── B7. Барьер наблюдает СОБСТВЕННУЮ проводку ─────────────────────────────────────────
def check_b7(ci):
    job = job_of(ci, WIRE_JOB)
    if job is None:
        bad("B7", f"в `ci.yml` нет джоба `{WIRE_JOB}` — барьер никем не зовётся, и его красное "
                  f"не блокирует ничего. Наблюдение ОТСУТСТВИЯ, которое само не наблюдается, "
                  f"защиты не даёт (класс TD-106/TD-062: «гейт есть, не гейтит»)")
        return
    problems = []
    if not invokes(job, BARRIER):
        problems.append(f"джоб не ЗОВЁТ `{BARRIER}` в позиции команды (упоминание в `echo` "
                        f"вызовом не является)")
    if not invokes(job, PROBE):
        problems.append(f"джоб не ЗОВЁТ пробу `{PROBE}` — барьер без пробы не отличим от "
                        f"барьера, пропускающего всё")
    if job.get("continue-on-error") in (True, "true") or static_false(job.get("if")):
        problems.append("джоб обезврежен на своём уровне")
    for st in steps_of(job):
        if st.get("continue-on-error") in (True, "true") or static_false(st.get("if")):
            problems.append(f"шаг «{st.get('name') or st.get('uses')}» обезврежен")
    gate = job_of(ci, AGG_GATE) or {}
    needs = gate.get("needs")
    needs = [needs] if isinstance(needs, str) else (needs if isinstance(needs, list) else [])
    if WIRE_JOB not in [str(n) for n in needs]:
        problems.append(f"`{WIRE_JOB}` отсутствует в `{AGG_GATE}.needs` — агрегат стартует, "
                        f"не дождавшись джоба")
    guard = "\n".join(st["run"] for st in steps_of(gate) if isinstance(st.get("run"), str))
    if f"needs.{WIRE_JOB}.result" not in guard:
        problems.append(f"результат `{WIRE_JOB}` не участвует в fail-closed условии "
                        f"`{AGG_GATE}` — красный джоб не уронит «All checks passed»")
    if problems:
        for p in problems:
            bad("B7", p)
        return
    ok("B7", f"`{WIRE_JOB}` зовёт барьер и пробу, стоит в `{AGG_GATE}.needs` и участвует "
             f"в его условии")


# ─── B8. Concurrency ───────────────────────────────────────────────────────────────────
def check_b8(bb, dp):
    conc = bb.get("concurrency")
    if isinstance(conc, str):
        conc = {"group": conc}
    if not isinstance(conc, dict) or not str(conc.get("group") or "").strip():
        bad("B8", "нет секции `concurrency.group` — серия intra-chain push'ей (штатный режим "
                  "`gates.md` §8) копит очередь многоминутных сборок одной ветки")
        return
    group = str(conc["group"])
    # ПОРЯДОК ВАЖЕН: столкновение с группой деплоя проверяется ПЕРВЫМ. Такая группа заодно
    # не содержит `github.ref`, и проверка «по ref» перехватила бы её, назвав самое дорогое
    # последствие (отмена идущей выкатки) второстепенным — цвет верный, диагноз ложный.
    dp_conc = dp.get("concurrency")
    dp_group = str((dp_conc or {}).get("group") if isinstance(dp_conc, dict) else (dp_conc or ""))
    if dp_group and group == dp_group:
        bad("B8", f"`concurrency.group` совпадает с группой `deploy.yml` («{dp_group}»): при "
                  f"`cancel-in-progress: true` сборка ветки ОТМЕНИТ ИДУЩИЙ ДЕПЛОЙ, который "
                  f"намеренно неотменяем (иначе прод остаётся в промежуточном состоянии)")
        return
    if "github.ref" not in group:
        bad("B8", f"`concurrency.group` = «{group}» не привязана к `github.ref` — ветки "
                  f"начнут отменять сборки ДРУГ ДРУГА, и ни одна не будет достроена")
        return
    ok("B8", f"`concurrency.group` = «{group}» — по ref и не пересекается с deploy")


# ─── B9. Подделка required-контекста и права ───────────────────────────────────────────
def check_b9(bb, ci):
    ci_names = set(job_names(ci).values())
    clash = sorted(set(job_names(bb).values()) & ci_names)
    if clash:
        bad("B9", f"имя джоба {clash} совпадает с именем джоба `ci.yml`. Для `main` требуется "
                  f"КОНТЕКСТ «All checks passed», и контекст — это display-имя джоба ЛЮБОГО "
                  f"workflow на том же SHA: одноимённый джоб здесь производит требуемый "
                  f"контекст в обход агрегата, то есть путь ПОДДЕЛКИ merge-гейта")
        return
    def writes(node, where):
        perms = node.get("permissions")
        if isinstance(perms, str) and perms.strip() != "read-all":
            return f"{where}: `permissions: {perms}`"
        if isinstance(perms, dict):
            w = sorted(k for k, v in perms.items() if str(v).strip() == "write")
            if w:
                return f"{where}: право записи на {w}"
        return None
    for node, where in ((bb, "workflow"), (job_of(bb, REF_JOB) or {}, f"джоб {REF_JOB}")):
        why = writes(node, where)
        if why:
            bad("B9", f"{why} — сборке ветки права записи не нужны; ветку пушит кто угодно, "
                      f"и токен с записью там становится поверхностью атаки")
            return
    ok("B9", f"имена джобов не пересекаются с `ci.yml`; прав на запись не запрошено")


def main():
    bb, err = load(BB)
    if err:
        bad("B1", f"{err} — сборка ветки не существует как механизм")
        print()
        print("VERDICT: FAIL (1) — предмет барьера отсутствует; остальные проверки лишены смысла.")
        print("Механизм — .github/workflows/branch-build.yml; проба — scripts/tests/red_branch_build.sh.")
        return 1
    ok("B1", f"{BB_REL} на месте и разбирается")

    ci, cierr = load(CI)
    if cierr:
        bad("B1", f"эталон паритета недоступен: {cierr}")
        ci = {}
    dp, dperr = load(DP)
    if dperr:
        bad("B1", f"deploy.yml недоступен: {dperr}")
        dp = {}

    bb_name = str(bb.get("name") or "").strip()
    if not bb_name:
        bad("B1", "у workflow нет `name:` — его нельзя ни назвать в `workflow_run`, ни узнать "
                  "в списке прогонов")
        bb_name = "Branch build"

    check_b2(triggers(bb, "branch-build.yml"))
    check_b3(bb, ci)
    check_b4(bb)
    check_b5(ci)
    check_b6(dp, bb_name)
    check_b7(ci)
    check_b8(bb, dp)
    check_b9(bb, ci)

    print()
    if FAILED:
        print(f"VERDICT: FAIL ({FAILED}) — сборка ветки не гарантирована, разошлась с `main` "
              f"либо задевает чужое.")
        print("Обоснование — .github/workflows/branch-build.yml; проба — scripts/tests/red_branch_build.sh.")
        return 1
    print("VERDICT: PASS — ветка собирается тем же составом, что `main`; агрегат, прод и "
          "merge-гейт не задеты.")
    return 0


sys.exit(main())
PY
