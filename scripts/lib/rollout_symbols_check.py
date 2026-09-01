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


def check_epoch(raw: str) -> list[str]:
    """`EPOCH_ID`: та же политика литерала плюс непустота."""
    if SUBSTITUTION.search(raw):
        return [
            f"EPOCH_ID={raw!r} записан подстановкой — переопределяемая эпоха разъедется с "
            f"составом, и события двух составов станут машинно неразличимы (класс E-001)"
        ]
    if not raw.strip():
        return [f"EPOCH_ID={raw!r} пуст: граница эпохи обязана быть предъявимым фактом, а не "
                f"дефолтом по часам (E-002)"]
    return []


def main() -> int:
    """CLI для шага `T10c`: `<символы> <эпоха>` → печатает нарушения, код 1 при их наличии."""
    if len(sys.argv) != 3:
        print("usage: rollout_symbols_check.py <L2DELTA_CAPTURE_SYMBOLS> <EPOCH_ID>")
        return 2
    bad = check_symbols(sys.argv[1]) + check_epoch(sys.argv[2])
    if bad:
        print("; ".join(bad))
        return 1
    print("OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
