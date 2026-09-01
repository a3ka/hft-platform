#!/usr/bin/env python3
"""ЕДИНСТВЕННЫЙ авторитетный компаратор состава раскатки `M-45` (`C-202` B-2).

Зовётся ДВУМЯ потребителями, и это его смысл:
  · шаг `T10`  — на реальном `docker-compose.yml` дерева;
  · шаг `T10c` — на фикстурах (мутационная проба).

**Почему один файл, а не две проверки.** `C-202` B-2 предъявил: прежний `T10c` РЕДЕКЛАРИРОВАЛ
разбор и сравнение вместо того, чтобы исполнить `T10`. Замер критика воспроизведён мной:
ослабление реального `T10` (`if got != SIGNED` → `if not got`) оставляло все десять сценариев
`T10c` ЗЕЛЁНЫМИ. Проба проверяла свою копию логики и не пиннила ничего.

Пока компаратор один, ослабить его незаметно нельзя: любое изменение здесь меняет исход
ОБОИХ шагов.

## Политика формы — литерал, а не подстановка (`C-202` B-1/B-3)

`T10` прежней редакции читал YAML-ТЕКСТ и принимал `${L2DELTA_CAPTURE_SYMBOLS:-BTCUSDT,ETHUSDT}`
как подписанный состав. Замер (`docker compose config`):

    L2DELTA_CAPTURE_SYMBOLS=BTCUSDT,ETHUSDT,SOLUSDT  →  recorder получает ТРИ символа
    без переменной                                    →  recorder получает два

То есть гейт границы C обходился одной переменной окружения, а `docker` в CI отсутствует
(`grep -c docker .github/workflows/ci.yml` → 0), и проверять эффективную конфигурацию его
средствами нельзя.

**Развязка — запретить подстановку для величины под подписью.** Литерал не переопределяется
ни окружением, ни `.env` — проверено обоими способами:

    literal + L2DELTA_CAPTURE_SYMBOLS=…,SOLUSDT  →  BTCUSDT,ETHUSDT
    literal + .env с тем же значением            →  BTCUSDT,ETHUSDT

Цена названа: оператор больше не может сменить СОСТАВ переменной окружения — только коммитом.
Это не потеря, а требование границы C: состав данных меняет подпись, а не рычаг под рукой.
`EPOCH_ID` подчиняется тому же правилу — эпоха, разъехавшаяся с составом, делает события двух
составов машинно неразличимыми (класс `E-001`).
"""

import re
import sys

# Подписано `П-026` (2026-08-31): ровно два инструмента, на обеих площадках Binance.
SIGNED = {"BTCUSDT", "ETHUSDT"}

# Любая форма интерполяции compose: `${VAR}`, `${VAR:-def}`, `${VAR-def}`, `${VAR:?err}`, `$VAR`.
SUBSTITUTION = re.compile(r"\$\{[^}]*\}|\$[A-Za-z_][A-Za-z0-9_]*")


def check_symbols(raw: str) -> list[str]:
    """Вернуть список нарушений состава. Пустой список = состав ПРИНЯТ."""
    if SUBSTITUTION.search(raw):
        return [
            f"L2DELTA_CAPTURE_SYMBOLS={raw!r} записан ПОДСТАНОВКОЙ. Величина под подписью "
            f"обязана быть ЛИТЕРАЛОМ: подстановка переопределяется переменной окружения или "
            f".env, и тогда recorder получит состав, которого подпись не давала (C-202 B-1). "
            f"Требуется ровно: BTCUSDT,ETHUSDT"
        ]
    got = {t.strip().upper() for t in raw.split(",") if t.strip()}
    if got == SIGNED:
        return []
    extra, missing = sorted(got - SIGNED), sorted(SIGNED - got)
    parts = []
    if extra:
        parts.append("ЛИШНИЕ (неподписанное расширение границы C): " + ", ".join(extra))
    if missing:
        parts.append("ОТСУТСТВУЮТ (сужение состава без подписи): " + ", ".join(missing))
    return [f"L2DELTA_CAPTURE_SYMBOLS={raw!r} не равен подписанному {sorted(SIGNED)} — "
            + "; ".join(parts)]


# Конвенция ОБЪЯВЛЕННОЙ эпохи — `CT-RFC-06` §3: `own-YYYY-MM-m45-<slug>`. Дефолт кода —
# `own-<UTC-YYYY-MM>` (`crates/recorder/src/main.rs`, `default_epoch_id_now`), то есть
# `own-2026-08`. Формы РАЗЛИЧАЮТСЯ наличием сегмента `m45-<slug>`, и это единственный
# машинный признак «эпоху объявили» против «эпоха досталась от часов».
DELIBERATE_EPOCH = re.compile(r"^own-\d{4}-\d{2}-m45-[a-z0-9][a-z0-9-]*$")


def check_epoch(raw: str) -> list[str]:
    """`EPOCH_ID`: литерал, непустой И ОБЪЯВЛЕННЫЙ, а не доставшийся от часов.

    Третья обязанность добавлена по `R-167` Б-3. Прежняя редакция держала только «не
    подстановка и не пусто», и замер ревьюера предъявил дыру: `EPOCH_ID: own-2026-08` —
    ДЕЙСТВУЮЩАЯ де-факто эпоха — проходил с `exit=0` при расширенном составе. То есть
    «состав расширили, метку не сменили» считалось исполнением, хотя докстринг этого же
    файла объявлял класс `E-001` закрытым. Оракул обещал больше, чем мерил
    (`testing.md` §«Оракул обязан мерить ТО, ЧТО ОБЕЩАЕТ»).
    """
    if SUBSTITUTION.search(raw):
        return [
            f"EPOCH_ID={raw!r} записан подстановкой — переопределяемая эпоха разъедется с "
            f"составом, и события двух составов станут машинно неразличимы (класс E-001)"
        ]
    if not raw.strip():
        return [f"EPOCH_ID={raw!r} пуст: граница эпохи обязана быть предъявимым фактом, а не "
                f"дефолтом по часам (E-002)"]
    if not DELIBERATE_EPOCH.match(raw.strip()):
        return [f"EPOCH_ID={raw!r} не является ОБЪЯВЛЕННОЙ эпохой: конвенция `CT-RFC-06` §3 — "
                f"own-YYYY-MM-m45-<slug>. Форма вида own-2026-08 неотличима от дефолта кода "
                f"по часам, то есть состав расширен, а граница эпохи не объявлена (E-001)"]
    return []


def read_env(path: str) -> tuple[int, object]:
    """Что видит в фикстуре ПОТРЕБИТЕЛЬ этого файла — то есть сам CLI.

    Вынесено отдельно и предъявляется наружу режимом `--extract` РАДИ SETUP-GUARD'А ПРОБЫ
    (`testing.md` §«Целостность гейта» св. 3: проба, молча тестирующая не тот сценарий, есть
    плацебо самой себя). Пока этой функции не было, `T10c` строила фикстуру ДОПИСЫВАНИЕМ
    ключа и никогда не проверяла, что дописанное вообще доехало до разбора.

    Замер, стоивший красного гейта на первой же раскатке (найдено engine-dev'ом на
    `f3b84d4`): после того как задача 7 внесла ключи в compose, дописка мутации ниже якоря
    `HL_COINS` создавала ДУБЛЬ ключа, а PyYAML применяет last-wins — побеждало реальное
    значение, мутация исчезала, и проба краснела на пяти сценариях из семи. То есть оракул
    был годен ТОЛЬКО в мире до раскатки и ломался ровно в тот момент, ради которого написан.

    Guard снимается ЭТИМ ЖЕ путём, каким читает судимый шаг (`Р-1`: мера на границе
    потребителя, не редекларация разбора в пробе — та ошибка уже стоила `C-202` B-2).

    Возврат: `(0, {ключ: сырое значение})` — разобрано; `(2, причина)` — setup не состоялся.
    """
    try:
        import yaml
    except Exception as e:  # pragma: no cover — среда без pyyaml
        return 2, f"SETUP: pyyaml недоступен ({e})"
    try:
        doc = yaml.safe_load(open(path))
    except Exception as e:
        return 2, f"SETUP: {path} не разобран: {e}"
    svcs = (doc or {}).get("services") or {}
    rec = [v for v in svcs.values()
           if isinstance(v, dict) and v.get("container_name") == "hft-recorder"]
    if len(rec) != 1:
        return 2, (f"SETUP: сервис с container_name=hft-recorder найден {len(rec)} раз — "
                   f"судить нечего")
    env = rec[0].get("environment") or {}
    if isinstance(env, list):
        env = dict(x.split("=", 1) for x in env if "=" in x)
    return 0, {k: str(env[k]) for k in ("L2DELTA_CAPTURE_SYMBOLS", "EPOCH_ID") if k in env}


def check_compose(path: str) -> tuple[int, str]:
    """Суждение о ФАЙЛЕ compose целиком (`A-030` §3 п.1).

    Сюда перенесена ВСЯ логика шага `T10`: разбор YAML, поиск сервиса, извлечение окружения,
    проверка наличия ключей и самих значений. Вне пробы остаётся только маппинг кода возврата
    на pass/fail — слой, общий для всех шагов `verify` и видимый одним экраном.

    **Зачем перенос.** `A-030` §3 замер 4b: пока склейка жила в `verify_M-45.sh`, мутация
    `bad = check_symbols(...) + check_epoch(...)` → `bad = []` пропускала литерал
    `BTCUSDT,ETHUSDT,SOLUSDT` с `exit=0`, а проба `T10c` оставалась зелёной 13/13. Проба
    мерила УЧАСТНИКА (компаратор), а не границу ПОТРЕБИТЕЛЯ (шаг, как его исполняет verify) —
    правило `Р-1`. Тот же класс ловился трижды, каждый раз уровнем выше; здесь он закрыт на
    последнем уровне, где выше только общий pass/fail-слой.

    Возврат: `(0, отчёт)` — принято; `(1, нарушения)` — отвергнуто; `(2, причина)` — SETUP не
    состоялся (fail-closed: молчать при несостоявшейся проверке нельзя).
    """
    code, got = read_env(path)
    if code != 0:
        return code, str(got)
    env = got
    miss = [k for k in ("L2DELTA_CAPTURE_SYMBOLS", "EPOCH_ID") if k not in env]
    if miss:
        return 1, "ОТСУТСТВУЮТ на сервисе recorder: " + ", ".join(miss)
    sym, epoch = str(env["L2DELTA_CAPTURE_SYMBOLS"]), str(env["EPOCH_ID"])
    bad = check_symbols(sym) + check_epoch(epoch)
    if bad:
        return 1, "; ".join(bad)
    return 0, f"OK L2DELTA_CAPTURE_SYMBOLS={sym} EPOCH_ID={epoch}"


def main() -> int:
    """Два режима, и оба ведут в ОДИН код суждения.

      --compose <путь>            — режим шага `T10` и фикстурных копий в `T10c`
      --extract <путь>            — что CLI ВИДИТ в файле; setup-guard пробы
      <символы> <эпоха>           — прямой режим для сценариев значений в `T10c`
    """
    if len(sys.argv) == 3 and sys.argv[1] == "--compose":
        code, msg = check_compose(sys.argv[2])
        print(msg)
        return code
    if len(sys.argv) == 3 and sys.argv[1] == "--extract":
        code, got = read_env(sys.argv[2])
        if code != 0:
            print(got)
            return code
        for k in ("L2DELTA_CAPTURE_SYMBOLS", "EPOCH_ID"):
            print(f"{k}={got[k]}" if k in got else f"{k}=<ОТСУТСТВУЕТ>")
        return 0
    if len(sys.argv) == 3:
        bad = check_symbols(sys.argv[1]) + check_epoch(sys.argv[2])
        if bad:
            print("; ".join(bad))
            return 1
        print("OK")
        return 0
    print("usage: rollout_symbols_check.py --compose <path> | --extract <path> "
          "| <SYMBOLS> <EPOCH_ID>")
    return 2


if __name__ == "__main__":
    sys.exit(main())
