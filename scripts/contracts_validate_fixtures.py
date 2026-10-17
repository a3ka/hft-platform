#!/usr/bin/env python3
"""Валидирует crates/contracts/fixtures/{valid,invalid} против crates/contracts/schema/*.json
через РЕАЛЬНЫЙ JSON Schema валидатор (`jsonschema`), а не serde-парсинг Rust-типов — это
проверка 2 из scripts/verify_contracts.sh (docs/05-contract-layer.md §5).

valid/* ОБЯЗАНЫ пройти; invalid/* ОБЯЗАНЫ быть ОТВЕРГНУТЫ. Второе важнее первого:
`crates/contracts/tests/red_schema.rs` уже проверяет то же самое serde-десериализацией
Rust-типов напрямую (компилятор помогает); этот скрипт проверяет НЕЗАВИСИМЫЙ путь —
СГЕНЕРИРОВАННУЮ схему саму по себе, как её увидит внешний (не-Rust) консюмер (CT-I-5:
"Python-тулинг валидирует чтения против той же схемы"). Фикстура, которая должна быть
отвергнута схемой и проходит, означает, что схема ничего не проверяет для этого класса
дефекта — то же анти-плацебо правило, что и для RED-тестов (.claude/rules/testing.md).
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

try:
    from jsonschema import Draft7Validator
except ImportError:  # pragma: no cover — пойман S0 setup-guard в verify_contracts.sh
    print("FAIL setup: python3 модуль jsonschema не установлен (pip install jsonschema)")
    sys.exit(1)

ROOT = Path(__file__).resolve().parent.parent
SCHEMA_DIR = ROOT / "crates/contracts/schema"
FIXTURES_DIR = ROOT / "crates/contracts/fixtures"

# Соответствие имя-файла-фикстуры → файл схемы (зеркалит логику
# `crates/contracts/tests/red_schema.rs::ct_rfc02_fixtures_valid_parse_invalid_reject`,
# которую я НЕ трогаю — тесты крейта contracts sacred/architect-only).
PREFIX_TO_SCHEMA = [
    ("segment-header", "segment-header.schema.json"),
    ("legacy-manifest", "legacy-manifest.schema.json"),
]
DEFAULT_SCHEMA = "event.schema.json"


def schema_file_for(fixture_name: str) -> str:
    for prefix, schema_name in PREFIX_TO_SCHEMA:
        if fixture_name.startswith(prefix):
            return schema_name
    return DEFAULT_SCHEMA


def load_validator(cache: dict[str, Draft7Validator], schema_name: str) -> Draft7Validator:
    if schema_name not in cache:
        schema_path = SCHEMA_DIR / schema_name
        if not schema_path.is_file():
            raise FileNotFoundError(f"схема отсутствует: {schema_path}")
        doc = json.loads(schema_path.read_text(encoding="utf-8"))
        cache[schema_name] = Draft7Validator(doc)
    return cache[schema_name]


def main() -> int:
    cache: dict[str, Draft7Validator] = {}
    failures: list[str] = []

    valid_dir = FIXTURES_DIR / "valid"
    invalid_dir = FIXTURES_DIR / "invalid"
    valid_files = sorted(valid_dir.glob("*.json"))
    invalid_files = sorted(invalid_dir.glob("*.json"))

    # Setup-guard (пустой каталог = ничего не проверено = FAIL, не тихий пропуск).
    if not valid_files:
        print(f"FAIL setup: {valid_dir} пуст — нечего проверять")
        return 1
    if not invalid_files:
        print(f"FAIL setup: {invalid_dir} пуст — нечего проверять")
        return 1

    for path in valid_files:
        schema_name = schema_file_for(path.name)
        try:
            validator = load_validator(cache, schema_name)
        except FileNotFoundError as exc:
            failures.append(str(exc))
            continue
        doc = json.loads(path.read_text(encoding="utf-8"))
        errors = sorted(validator.iter_errors(doc), key=lambda e: e.message)
        if errors:
            failures.append(
                f"valid-фикстура {path.name} ОТВЕРГНУТА схемой {schema_name} (обязана "
                f"проходить): {errors[0].message}"
            )

    for path in invalid_files:
        schema_name = schema_file_for(path.name)
        try:
            validator = load_validator(cache, schema_name)
        except FileNotFoundError as exc:
            failures.append(str(exc))
            continue
        doc = json.loads(path.read_text(encoding="utf-8"))
        errors = list(validator.iter_errors(doc))
        if not errors:
            failures.append(
                f"invalid-фикстура {path.name} ПРОШЛА схему {schema_name} невредимой (обязана "
                f"быть ОТВЕРГНУТА — схема ничего не проверяет на этом кейсе, CT-I-4 профанация)"
            )

    if failures:
        for f in failures:
            print("FAIL", f)
        print(
            f"итог: {len(failures)} нарушени(й) из {len(valid_files)} valid + "
            f"{len(invalid_files)} invalid фикстур"
        )
        return 1

    print(
        f"OK: {len(valid_files)} valid-фикстур прошли схему, "
        f"{len(invalid_files)} invalid-фикстур отвергнуты ({SCHEMA_DIR})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
