#!/usr/bin/env python3
"""Сторож «деплой не состоялся» — механизм Р-4 (docs/plans/process-decisions-2026-08-14.md).

Замер, ради которого механизм существует (14.08 + повтор 17.08):

    прод   362784a (14.08 21:16Z)      main   2c56a34 (17.08)
    кодовая дельта — 2 файла, 92 коммита позади
    последний Deploy-ран 61f452e: `Gate on CI` = failure, `Deploy (build on VPS)` = SKIPPED

Класс дыры — «наблюдается сбой, НЕ наблюдается ОТСУТСТВИЕ» (`testing.md`, свойство 4):
push-фильтр `deploy.yml` смотрит на кодовые пути, а фикс, зеленящий CI, лежит вне них
(`scripts/tests/**` в `paths` не входит ВОВСЕ — `TD-150` п.2; отрицательный паттерн там
ровно один, `!crates/*/tests/**`). Значит НИ ОДИН push в цепочке не обязан выпустить прод,
и упавший ран сам себя не перезапускает.

Две подкоманды:

  decide        — рантайм-решение: DEPLOY / SKIP / HOLD. Зовётся из `deploy.yml`
                  на событии `workflow_run` (CI на main завершился success).
  check-wiring  — статический барьер на проводку `deploy.yml`: catch-up подключён,
                  а запретный список Р-4 не нарушен.

## Различитель, ради которого всё это (Р-4, строки 296-301)

Авто-добор — ТОЛЬКО для класса «деплой не СТАРТОВАЛ» (CI-гейт красен / фильтр путей не
совпал). Деплой, уже УПАВШИЙ на VPS-шаге для той же вершины, автоматом НЕ повторяется:
авто-ретрай детерминированной ошибки молотил бы прод rollback-циклами, а `fa/ops.md` §5.1
прямо говорит, что авто-rollback против schema-forward журнала «НЕ применять вслепую».
Там остаётся человек и §8 «немедленный фикс или revert».

Направление fail-closed выбрано так, что автоматика требует ПОЛОЖИТЕЛЬНОГО доказательства
безопасности: DEPLOY выдаётся, только когда про КАЖДЫЙ ран этой вершины удалось показать,
что VPS-джоб не исполнялся. Ран, который не удаётся классифицировать (джоба нет в списке —
переименование, обрезанный ответ API), даёт HOLD, а не DEPLOY. Обратное направление —
«не смогли разобрать ⇒ поехали» — воспроизводило бы ровно тот дефект, который Р-4 запрещает.

## SHA-якорность (TD-150 п.1, severity MAJOR «в момент написания спек»)

Решение принимается и исполняется НАД КОНКРЕТНЫМ SHA, а не над веткой. `gh workflow run
deploy.yml --ref main` привязан к ВЕТКЕ: между зеленением CI и добором `main` уезжает вперёд
(14.08 — 13 коммитов за день), и выкатывается не та вершина, чей CI проверялся. Здесь целевой
SHA приходит явно и проверяется на форму; `deploy.yml` ресетит VPS на него же.
"""

import fnmatch
import json
import os
import re
import subprocess
import sys

SHA_RE = re.compile(r"^[0-9a-f]{40}$")

# Решения. Ровно три, и они исчерпывают пространство: молчание — не решение.
DEPLOY = "DEPLOY"  # добор оправдан: вершина должна быть на VPS и не доехала
SKIP = "SKIP"      # добирать нечего (уже там / нет кодовой дельты / ран в полёте)
HOLD = "HOLD"      # человеку: VPS трогали и он упал, либо ран не классифицируется

# Код возврата для HOLD — НЕНУЛЕВОЙ и ОТЛИЧИМЫЙ от отказа входа (`Fail` ⇒ 1).
#
# Почему не 0 (находка `C-093` R-2). `emit()` печатал решение и возвращал 0 на всех путях,
# поэтому HOLD давал ЗЕЛЁНЫЙ job `catchup`, skipped `deploy` и терминально зелёный ран
# «Deploy to VPS» — состояние, ВНЕШНЕ НЕОТЛИЧИМОЕ от успешной доставки. Автодобора при этом
# верно не происходило (Р-4), но и человек не узнавал ничего: сторож наблюдал сбой и не
# наблюдал ОТСУТСТВИЕ (`testing.md`, целостность гейта, свойство 4).
#
# Почему не 1. Единица уже занята `Fail` — «вход не разобран, решения нет вовсе». Слить их
# значило бы сделать «сторож сработал и зовёт человека» неотличимым от «сторож сломался»,
# а это разные состояния с разными действиями оператора.
#
# ПРЕДЕЛ ЭТОГО КАНАЛА НАЗВАН, А НЕ ЗАМАСКИРОВАН. `docs/plans/process-decisions-2026-08-14.md`
# §Варианты(б) отверг сторож-алерт словами «красный джоб в Actions никто не смотрит — это та
# же слепота, что сейчас; Telegram — no-op без токена». Здесь тот же класс канала, и он
# выбран НЕ потому, что хорош, а потому, что это единственное, что доступно в периметре:
# `issues: write` потребовало бы расширения прав `GITHUB_TOKEN`, прямо запрещённого запретным
# списком Р-4 (замер: `deploy.yml` permissions = actions:read + contents:read). Достигнутое
# улучшение назовём точно: переход из НЕНАБЛЮДАЕМОГО (зелёный ран = успех) в СЛАБО
# НАБЛЮДАЕМОЕ (красный ран виден в `gh run list --workflow=deploy.yml`, который architect
# снимает каждой сессией — ярус S startup-протокола). Доставляемый алерт остаётся за `П-003`.
HOLD_RC = 2


class Fail(Exception):
    """Вход не разобран. Любой такой случай — exit 1, и решение НЕ выдаётся вовсе."""


# ---------------------------------------------------------------- deploy.yml

def load_workflow(path):
    try:
        import yaml
    except ImportError as exc:  # pragma: no cover - окружение без PyYAML
        raise Fail(f"PyYAML недоступен ({exc}) — разобрать deploy.yml нечем") from exc
    try:
        with open(path, encoding="utf-8") as fh:
            wf = yaml.safe_load(fh)
    except OSError as exc:
        raise Fail(f"deploy.yml не читается: {exc}") from exc
    except Exception as exc:
        raise Fail(f"deploy.yml не разбирается как YAML: {exc}") from exc
    if not isinstance(wf, dict):
        raise Fail(f"deploy.yml: ожидался маппинг, получено {type(wf).__name__}")
    return wf


def triggers(wf):
    """`on:` в YAML 1.1 — булев ключ True, а не строка 'on'. Обе формы, иначе гейт слеп."""
    for key in ("on", True):
        if key in wf:
            node = wf[key]
            if isinstance(node, dict):
                return node
            raise Fail(f"deploy.yml: `on:` не маппинг, а {type(node).__name__}")
    raise Fail("deploy.yml: секции `on:` нет вовсе")


def push_paths(wf):
    """Фильтр кодовых путей берётся ИЗ deploy.yml, а не переписывается рядом.

    Дублирование списка дало бы дрейф двух строк — класс «producer пишет туда, consumer
    читает отсюда» (`testing.md`, канарейка ops-пути, п.2). Здесь он закрыт по построению:
    источник один.
    """
    push = triggers(wf).get("push")
    if not isinstance(push, dict):
        raise Fail("deploy.yml: `on.push` отсутствует или не маппинг — фильтр путей не определён")
    paths = push.get("paths")
    if not isinstance(paths, list) or not paths:
        raise Fail("deploy.yml: `on.push.paths` пуст или отсутствует — кодовая дельта неопределима")
    out = []
    for item in paths:
        if not isinstance(item, str) or not item:
            raise Fail(f"deploy.yml: негодный элемент paths: {item!r}")
        out.append(item)
    return out


def path_matches(path, patterns):
    """Семантика GitHub: побеждает ПОСЛЕДНИЙ совпавший паттерн; `!` — отрицание.

    Комментарий в самом `deploy.yml` (строки 38-41) на это опирается: смешанный коммит
    (`src` + `tests`) деплой ВСЁ ЕЩЁ триггерит, потому что `crates/**` стоит раньше
    `!crates/*/tests/**` и для файла из `src` последним совпадает именно он.
    """
    verdict = False
    for pattern in patterns:
        negated = pattern.startswith("!")
        glob = pattern[1:] if negated else pattern
        if fnmatch.fnmatchcase(path, glob) or _github_doublestar(path, glob):
            verdict = not negated
    return verdict


def _github_doublestar(path, glob):
    """`fnmatch` не различает `*` и `**`: `crates/*/tests/**` у него совпал бы с
    `crates/a/b/tests/c`. Различие здесь несущее — на нём стоит `TD-086` (push тестов
    не должен передеплоивать прод), поэтому `**` разбирается явно."""
    parts = glob.split("/")
    target = path.split("/")

    def walk(pi, ti):
        if pi == len(parts):
            return ti == len(target)
        part = parts[pi]
        if part == "**":
            for skip in range(ti, len(target) + 1):
                if walk(pi + 1, skip):
                    return True
            return False
        if ti == len(target):
            return False
        if not fnmatch.fnmatchcase(target[ti], part):
            return False
        return walk(pi + 1, ti + 1)

    return walk(0, 0)


# ---------------------------------------------------------------- git-дельта

def git(root, *args):
    proc = subprocess.run(
        ["git", "-C", root, *args], capture_output=True, text=True, check=False
    )
    if proc.returncode != 0:
        raise Fail(f"git {' '.join(args)} → exit={proc.returncode}: {proc.stderr.strip()}")
    return proc.stdout


def code_delta(root, deployed, target, patterns):
    """Файлы, изменившиеся между тем, ЧТО НА VPS, и целевой вершиной, — под фильтром кода.

    База сравнения — задеплоенный SHA, а НЕ предыдущий push. Именно это и убирает класс по
    построению: выпуск перестаёт зависеть от того, КАКОЙ push сделал CI зелёным (Р-4, (в)).
    """
    for sha in (deployed, target):
        try:
            git(root, "cat-file", "-e", f"{sha}^{{commit}}")
        except Fail as exc:
            raise Fail(f"ревизия {sha[:12]} недоступна в этой истории: {exc}") from exc
    raw = git(root, "diff", "--name-only", deployed, target)
    changed = [line for line in raw.splitlines() if line.strip()]
    return [p for p in changed if path_matches(p, patterns)]


# ---------------------------------------------------------------- различитель

def vps_job_name(wf):
    jobs = wf.get("jobs")
    if not isinstance(jobs, dict) or "deploy" not in jobs:
        raise Fail("deploy.yml: job `deploy` отсутствует — различитель не на что опереть")
    node = jobs["deploy"]
    if not isinstance(node, dict):
        raise Fail("deploy.yml: job `deploy` не маппинг")
    name = node.get("name")
    if not isinstance(name, str) or not name.strip():
        raise Fail("deploy.yml: у job `deploy` нет `name` — VPS-джоб в ранах неопознаваем")
    return name.strip()


def load_runs(path):
    try:
        with open(path, encoding="utf-8") as fh:
            runs = json.load(fh)
    except OSError as exc:
        raise Fail(f"файл ранов не читается: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise Fail(f"файл ранов не разбирается как JSON: {exc}") from exc
    if not isinstance(runs, list):
        raise Fail(f"файл ранов: ожидался список, получено {type(runs).__name__}")
    for run in runs:
        if not isinstance(run, dict):
            raise Fail(f"элемент списка ранов не объект: {run!r}")
    return runs


def classify_runs(runs, target, job_name):
    """Что известно про Deploy-раны ЭТОЙ вершины.

    Возвращает (decision_or_None, reason). None ⇒ ни один ран не запрещает добор.
    """
    for run in runs:
        head = str(run.get("headSha") or run.get("head_sha") or "")
        if head and head != target:
            raise Fail(
                f"ран {run.get('databaseId')} несёт headSha={head[:12]}, а цель {target[:12]} — "
                "выборка собрана не по той вершине"
            )

    for run in runs:
        status = str(run.get("status") or "")
        if status and status != "completed":
            return SKIP, f"Deploy-ран {run.get('databaseId')} ещё в полёте (status={status})"

    for run in runs:
        jobs = run.get("jobs")
        if not isinstance(jobs, list):
            return HOLD, (
                f"ран {run.get('databaseId')} не несёт списка джобов — классифицировать "
                "«стартовал/не стартовал» нечем, решает человек"
            )
        vps = [j for j in jobs if isinstance(j, dict) and str(j.get("name", "")).strip() == job_name]
        if not vps:
            return HOLD, (
                f"в ране {run.get('databaseId')} нет джоба {job_name!r} (переименование? "
                "обрезанный ответ API) — VPS мог быть тронут, решает человек"
            )
        for job in vps:
            concl = job.get("conclusion")
            concl = str(concl) if concl is not None else "none"
            if concl == "failure":
                return HOLD, (
                    f"ран {run.get('databaseId')}: {job_name} УПАЛ на VPS — авто-ретрай "
                    "детерминированной ошибки запрещён (Р-4), решает человек"
                )
            if concl == "success":
                return HOLD, (
                    f"ран {run.get('databaseId')}: {job_name} успешен для этой вершины, но на "
                    "VPS её нет — состояние VPS менялось мимо пайплайна, решает человек"
                )
            # Безопасен РОВНО ОДИН исход — `skipped`: джоб не запускался, VPS не трогали.
            # `cancelled` сюда не входит намеренно: отмена могла прийти посреди
            # `docker compose up`, и состояние VPS тогда промежуточное. Всё, что не
            # доказывает «не стартовал», — человеку.
            if concl != "skipped":
                return HOLD, (
                    f"ран {run.get('databaseId')}: {job_name} conclusion={concl} — не доказано, "
                    "что VPS не трогали, решает человек"
                )
    return None, ""


# ---------------------------------------------------------------- decide

def emit(decision, reason):
    print(f"decision={decision}")
    print(f"reason={reason}")
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a", encoding="utf-8") as fh:
            fh.write(f"decision={decision}\n")
            fh.write(f"reason={reason}\n")


def need_sha(name):
    raw = (os.environ.get(name) or "").strip()
    if not SHA_RE.match(raw):
        raise Fail(f"{name}={raw!r} — нужен полный 40-символьный hex-SHA (SHA-якорность, TD-150 п.1)")
    return raw


def cmd_decide():
    root = os.environ.get("CATCHUP_REPO_ROOT") or "."
    wf_path = os.environ.get("CATCHUP_DEPLOY_YML") or os.path.join(
        root, ".github", "workflows", "deploy.yml"
    )
    runs_path = (os.environ.get("CATCHUP_RUNS_JSON") or "").strip()
    if not runs_path:
        raise Fail("CATCHUP_RUNS_JSON не задан — история Deploy-ранов не предъявлена")

    target = need_sha("CATCHUP_TARGET_SHA")
    deployed = need_sha("CATCHUP_DEPLOYED_SHA")
    wf = load_workflow(wf_path)
    job_name = vps_job_name(wf)
    patterns = push_paths(wf)
    runs = load_runs(runs_path)

    if target == deployed:
        emit(SKIP, f"VPS уже на целевой вершине {target[:12]}")
        return 0

    delta = code_delta(root, deployed, target, patterns)
    if not delta:
        emit(
            SKIP,
            f"между {deployed[:12]} и {target[:12]} нет кодовой дельты под фильтром deploy.yml "
            "— рестарт рекордера (гэп forward-only записи, TD-086) не оправдан",
        )
        return 0

    verdict, reason = classify_runs(runs, target, job_name)
    if verdict is not None:
        # emit() ДО возврата и в обоих случаях: `decision` обязан доехать до GITHUB_OUTPUT
        # даже когда шаг падает, иначе `deploy.if` читает пустой output. Пустой ≠ 'DEPLOY',
        # то есть fail-closed сохраняется и при потере вывода — но терять его незачем.
        emit(verdict, reason)
        return HOLD_RC if verdict == HOLD else 0

    emit(
        DEPLOY,
        f"кодовая дельта {len(delta)} файл(ов) (первый: {delta[0]}); ни один Deploy-ран "
        f"вершины {target[:12]} не исполнял {job_name!r} — класс «деплой не стартовал»",
    )
    return 0


# ---------------------------------------------------------------- check-wiring

def _text(node):
    return node if isinstance(node, str) else json.dumps(node, ensure_ascii=False)


def cmd_check_wiring():
    root = os.environ.get("CATCHUP_REPO_ROOT") or "."
    wf_path = os.environ.get("CATCHUP_DEPLOY_YML") or os.path.join(
        root, ".github", "workflows", "deploy.yml"
    )
    wf = load_workflow(wf_path)
    jobs = wf.get("jobs")
    if not isinstance(jobs, dict):
        raise Fail("deploy.yml: секции `jobs` нет")
    problems = []

    def bad(code, msg):
        problems.append(f"{code}: {msg}")

    # W1 — catch-up вообще подключён к завершению CI на main.
    wr = triggers(wf).get("workflow_run")
    if not isinstance(wr, dict):
        bad("W1", "нет триггера `on.workflow_run` — catch-up не подключён ни к чему")
    else:
        if wr.get("workflows") != ["CI"]:
            bad("W1", f"on.workflow_run.workflows = {wr.get('workflows')!r}, ожидалось ['CI']")
        if wr.get("branches") != ["main"]:
            bad("W1", f"on.workflow_run.branches = {wr.get('branches')!r}, ожидалось ['main']")
        if wr.get("types") != ["completed"]:
            bad("W1", f"on.workflow_run.types = {wr.get('types')!r}, ожидалось ['completed']")

    # W2 — джоб решения существует и не срабатывает на красном CI.
    catchup = jobs.get("catchup")
    if not isinstance(catchup, dict):
        bad("W2", "нет job `catchup` — решать «добирать или нет» некому")
    else:
        cond = _text(catchup.get("if") or "")
        if "workflow_run" not in cond:
            bad("W2", "job `catchup` не ограничен событием workflow_run")
        if "success" not in cond:
            bad(
                "W2",
                "job `catchup` не требует workflow_run.conclusion == 'success' — добор поверх "
                "красного CI (ослабление fail-closed гейта запрещено Р-4)",
            )

    # W3/W4 — деплой физически не может поехать мимо решения.
    deploy = jobs.get("deploy")
    if not isinstance(deploy, dict):
        bad("W3", "нет job `deploy`")
    else:
        needs = deploy.get("needs")
        needs = [needs] if isinstance(needs, str) else (needs or [])
        for req in ("ci", "catchup"):
            if req not in needs:
                bad("W3", f"jobs.deploy.needs = {needs!r} — нет `{req}`")
        cond = _text(deploy.get("if") or "")
        if "needs.catchup.outputs.decision" not in cond or "'DEPLOY'" not in cond:
            bad(
                "W4",
                "jobs.deploy.if не требует needs.catchup.outputs.decision == 'DEPLOY' — на "
                "workflow_run деплой поедет без решения сторожа",
            )
        if "needs.ci.result" not in cond or "'success'" not in cond:
            bad(
                "W4",
                "jobs.deploy.if не требует needs.ci.result == 'success' — push-путь потерял "
                "fail-closed гейт на CI (M-08 E9)",
            )

    # W5 — TD-086: гонка деплоев и запрет отмены на полпути. Барьера на это не было вовсе.
    conc = wf.get("concurrency")
    if not isinstance(conc, dict):
        bad("W5", "секции `concurrency` нет — гонка деплоев (TD-086) ничем не удержана")
    else:
        if conc.get("group") != "deploy-main":
            bad("W5", f"concurrency.group = {conc.get('group')!r}, ожидалось 'deploy-main'")
        if conc.get("cancel-in-progress") is not False:
            bad(
                "W5",
                f"concurrency.cancel-in-progress = {conc.get('cancel-in-progress')!r} — отмена "
                "на полпути оставит прод в промежуточном состоянии (TD-086)",
            )

    # W6 — запретный список Р-4: права GITHUB_TOKEN не расширяются.
    perms = wf.get("permissions")
    if not isinstance(perms, dict):
        bad("W6", f"permissions = {perms!r} — нужен явный маппинг (TD-018)")
    else:
        allowed = {"actions": "read", "contents": "read"}
        if perms.get("actions") != "read":
            bad("W6", "нет `permissions.actions: read` — gate-job упрётся в 403 (TD-018)")
        for key, val in perms.items():
            if allowed.get(key) != val:
                bad(
                    "W6",
                    f"permissions.{key} = {val!r} — расширение прав GITHUB_TOKEN запрещено "
                    "запретным списком Р-4",
                )

    script = ""
    if isinstance(deploy, dict):
        script = json.dumps(deploy.get("steps"), ensure_ascii=False)

    # W7 — rollback-ветка не тронута (запретный список Р-4).
    if 'git reset --hard -q \\"$PREV\\"' not in script and "git reset --hard -q \"$PREV\"" not in script:
        bad("W7", "в шагах `deploy` нет отката на $PREV — rollback-ветка тронута (запрещено Р-4)")

    # W8 — гейт на CI не ослаблен.
    gate = jobs.get("ci")
    if not isinstance(gate, dict):
        bad("W8", "job `ci` (gate) отсутствует")
    elif "exit 1" not in json.dumps(gate, ensure_ascii=False):
        bad("W8", "gate-job `ci` не fail-closed: нет явного `exit 1`")

    # W9 — SHA-якорность (TD-150 п.1): выкатывается решённая вершина, а не «что на ветке».
    if "reset --hard -q origin/main" in script or "reset --hard origin/main" in script:
        bad(
            "W9",
            "шаги `deploy` ресетят VPS на origin/main — привязка к ВЕТКЕ: между зеленением CI "
            "и выкаткой main уезжает вперёд, и на прод едет не та вершина (TD-150 п.1)",
        )
    if "TARGET_SHA" not in script:
        bad("W9", "в шагах `deploy` нет TARGET_SHA — выкатка не SHA-якорная (TD-150 п.1)")

    if problems:
        for line in problems:
            print(f"FAIL {line}")
        print(f"VERDICT: FAIL ({len(problems)} нарушени(й) проводки)")
        return 1
    print(
        "OK: workflow_run→CI/main; catchup гейтит deploy; concurrency deploy-main/no-cancel; "
        "permissions не расширены; rollback и CI-гейт целы; выкатка SHA-якорная"
    )
    print("VERDICT: PASS")
    return 0


# ---------------------------------------------------------------- check-aggregate

# Барьер CI-АГРЕГАТА. Закрывает `C-093` R-1.
#
# Дыра, которую он закрывает, замерена критиком: проба знала ровно два файла — предмет и
# `deploy.yml` — а `ci.yml` не была в её универсуме ВООБЩЕ. Поэтому два независимых стаба
# («джоб выкинут из условия агрегата» и «выкинут и из `needs`, и из условия») сохраняли
# полный зелёный прогон 39/39: стаб и честная проводка были для пробы НЕОТЛИЧИМЫ.
#
# Класс известен и уже стоил семи красных прогонов `main`: джоб красен, а агрегат печатает
# «All checks passed» (`C-082` B-3). Барьер обязан проверять И `needs`, И участие в
# fail-closed условии — второе ПО ВЫЗОВУ, а не грепом: `grep` по имени джоба зелен и против
# закомментированной строки, и против упоминания в соседнем эхо (`testing.md`: «проверка
# должна быть по ВЫЗОВУ (исполнением/поведением), а не по тексту»).

AGG_JOB = "deploy-catchup"
AGG_GATE = "status-check"


def _agg_run_script(job):
    """Конкатенация всех `run` шагов джоба — то, что реально исполняет раннер."""
    out = []
    for step in job.get("steps") or []:
        if isinstance(step, dict) and isinstance(step.get("run"), str):
            out.append(step["run"])
    return "\n".join(out)


def _agg_expand(script, results):
    """Подставить `${{ needs.<job>.result }}` фактическими значениями модели."""
    def sub(m):
        return results.get(m.group(1), "success")
    return re.sub(r"\$\{\{\s*needs\.([A-Za-z0-9_-]+)\.result\s*\}\}", sub, script)


def _agg_exec(script, results, root):
    """ИСПОЛНИТЬ условие агрегата при заданной модели результатов. Возвращает (rc, stdout)."""
    body = _agg_expand(script, results)
    # Оставшиеся `${{ ... }}` (github.*, env.*) обнуляем: они не участвуют в решении
    # fail-closed условия, но синтаксически ломают bash.
    body = re.sub(r"\$\{\{[^}]*\}\}", "", body)
    proc = subprocess.run(
        ["bash", "-c", body], cwd=root, capture_output=True, text=True, timeout=60
    )
    return proc.returncode, (proc.stdout or "") + (proc.stderr or "")


def cmd_check_aggregate():
    root = os.environ.get("CATCHUP_REPO_ROOT") or "."
    ci_path = os.environ.get("CATCHUP_CI_YML") or os.path.join(
        root, ".github", "workflows", "ci.yml"
    )
    wf = load_workflow(ci_path)
    jobs = wf.get("jobs")
    if not isinstance(jobs, dict):
        raise Fail(f"{ci_path}: секции `jobs` нет")
    problems = []

    def bad(code, msg):
        problems.append(f"{code}: {msg}")

    # A1 — джоб существует и РЕАЛЬНО зовёт предмет, а не назван похоже.
    job = jobs.get(AGG_JOB)
    if not isinstance(job, dict):
        bad("A1", f"джоба `{AGG_JOB}` в {ci_path} нет — сторож не проводится в CI вовсе")
        script = ""
    else:
        script = _agg_run_script(job)
        if "deploy_catchup.py" not in script:
            bad("A1", f"джоб `{AGG_JOB}` не зовёт `scripts/deploy_catchup.py` — джоб-пустышка")
        if "red_deploy_catchup.sh" not in script:
            bad("A1", f"джоб `{AGG_JOB}` не гоняет пробу `scripts/tests/red_deploy_catchup.sh`")

    gate = jobs.get(AGG_GATE)
    if not isinstance(gate, dict):
        bad("A2", f"джоба-агрегата `{AGG_GATE}` в {ci_path} нет")
        gate = {}

    # A2 — зависимость закреплена: без `needs` агрегат стартует, не дождавшись джоба.
    needs = gate.get("needs")
    if isinstance(needs, str):
        needs = [needs]
    if not isinstance(needs, list) or AGG_JOB not in needs:
        bad("A2", f"`{AGG_JOB}` отсутствует в `{AGG_GATE}.needs` = {needs!r}")

    # A3 — УЧАСТИЕ В РЕШЕНИИ, проверенное ИСПОЛНЕНИЕМ.
    #
    # Две модели, и обе обязательны. Только «красный джоб ⇒ агрегат падает» прошёл бы
    # против условия `exit 1` без всяких условий («падать всегда»); только «всё зелено ⇒
    # агрегат проходит» прошёл бы против `exit 0` («не падать никогда»). Пара различает.
    gate_script = _agg_run_script(gate)
    if not gate_script:
        bad("A3", f"у `{AGG_GATE}` нет ни одного шага `run` — решать нечем")
    else:
        rc_red, out_red = _agg_exec(gate_script, {AGG_JOB: "failure"}, root)
        if rc_red == 0:
            bad(
                "A3",
                f"МОДЕЛЬ «{AGG_JOB}=failure, остальные success»: агрегат вернул exit=0 "
                f"({(out_red.strip().splitlines() or [''])[-1]!r}). Красный джоб не роняет "
                f"`{AGG_GATE}` — ветка вливается с неработающим сторожем",
            )
        rc_green, out_green = _agg_exec(gate_script, {}, root)
        if rc_green != 0:
            bad(
                "A3",
                f"АНТИ-ПЛАЦЕБО: при всех success агрегат вернул exit={rc_green} "
                f"({(out_green.strip().splitlines() or [''])[-1]!r}). Условие «падать всегда» "
                f"прошло бы первую модель вакуумно",
            )

    if problems:
        for line in problems:
            print(f"FAIL {line}")
        print(f"VERDICT: FAIL ({len(problems)} нарушени(й) CI-агрегата)")
        return 1
    print(
        f"OK: джоб `{AGG_JOB}` зовёт предмет и пробу; он в `{AGG_GATE}.needs`; "
        f"его красный результат РОНЯЕТ агрегат (проверено исполнением условия, не грепом)"
    )
    print("VERDICT: PASS")
    return 0


def main(argv):
    if len(argv) != 2 or argv[1] not in ("decide", "check-wiring", "check-aggregate"):
        print(f"usage: {argv[0]} decide|check-wiring|check-aggregate", file=sys.stderr)
        return 2
    try:
        return {
            "decide": cmd_decide,
            "check-wiring": cmd_check_wiring,
            "check-aggregate": cmd_check_aggregate,
        }[argv[1]]()
    except Fail as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        print("decision=", file=sys.stdout)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
