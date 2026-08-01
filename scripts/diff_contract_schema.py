#!/usr/bin/env python3
"""Классифицирует изменение crates/contracts/schema/*.json между base-ref и HEAD:
additive vs breaking (перенос принципа einhard `diff_schemas.py`,
docs/plans/contracts-einhard-inventory.md §1.5.2/§3 П2).

Правила (те же, что einhard §1.5.2, JSON Schema draft-07/2020-12-агностично):
  - удалённое свойство/тип/вариант enum          → BREAKING
  - новое `required` на СУЩЕСТВОВАВШЕМ свойстве   → BREAKING (было опционально, стало обязательно)
  - сужение enum (значение убрано)                → BREAKING
  - удалённый вариант `oneOf` (enum-вариант T1)   → BREAKING
  - смена `type` существующего поля               → BREAKING
  - `additionalProperties`: true/отсутствует → false → BREAKING
  - новое ОПЦИОНАЛЬНОЕ поле                       → ADDITIVE
  - новый вариант enum / `oneOf`                  → ADDITIVE
  - новый файл схемы (новый T1-тип)               → ADDITIVE
  - удалённый файл схемы (T1-тип убран целиком)   → BREAKING
  - `$ref` цель ссылки изменена (не разыменовывается) → BREAKING (консервативно, R-006 F-1)
  - файл изменился побайтово, но правила выше ничего не нашли → BREAKING (fail-closed
    safety-net; гейт никогда не утверждает "не изменилась" на изменённом файле, R-006 F-1)

Не переносим доменные §4d правила einhard (lat/lon/heading — не наша предметная область,
docs/plans/contracts-einhard-inventory.md §4). Это ЧИСТАЯ функция над двумя JSON-документами
(тот же паттерн, что их `diff_schemas.py` — газ-ref → git-ref, без внешних зависимостей).

Запуск как модуль (сравнение уже прочитанных словарей) — обёртка `diff_contract_schema.sh`
достаёт содержимое схем на двух git-ref через `git show` и передаёт сюда через CLI.
"""
from __future__ import annotations

import json
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Change:
    cls: str  # "breaking" | "additive"
    where: str


def _defs(doc: dict) -> dict:
    # schemars 0.8 использует draft-07 "definitions"; сохраняем совместимость с "$defs"
    # (draft 2020-12), если когда-нибудь схема поменяет диалект (CT-I-4 гейт это уже проверяет).
    return doc.get("definitions") or doc.get("$defs") or {}


def _oneof_variant_key(variant: dict) -> str | None:
    # schemars внешне-тегированный enum: {"required": ["VariantName"], "properties": {...}}.
    req = variant.get("required")
    if isinstance(req, list) and len(req) == 1:
        return req[0]
    # enum-подобный вариант без объекта (например {"enum": ["X"]}) — используем сам enum.
    if "enum" in variant and isinstance(variant["enum"], list) and len(variant["enum"]) == 1:
        return f"enum:{variant['enum'][0]}"
    return None


def classify_node(base: object, head: object, path: str) -> list[Change]:
    changes: list[Change] = []

    if not isinstance(base, dict) or not isinstance(head, dict):
        return changes

    # $ref (R-006 F-1): узел, представленный (полностью или частично) как {"$ref": "..."},
    # не имеет ни одного из ключей ниже (properties/type/enum/...), поэтому до фикса
    # перенацеливание $ref на другой существующий тип не давало НИ ОДНОГО Change → CLASS=none
    # → скрипт печатал ФАКТИЧЕСКИ ЛОЖНОЕ "схема не изменилась". Мы не разыменовываем ссылку
    # (чистая функция без каталога определений на этом уровне рекурсии) — трактуем ЛЮБОЕ
    # изменение цели ссылки КОНСЕРВАТИВНО как BREAKING: доказать безопасность retarget'а без
    # разыменования нельзя, а тихий "none" на реально изменённом узле хуже ложного breaking.
    base_ref = base.get("$ref") if isinstance(base.get("$ref"), str) else None
    head_ref = head.get("$ref") if isinstance(head.get("$ref"), str) else None
    if base_ref != head_ref:
        if base_ref is not None and head_ref is not None:
            changes.append(
                Change("breaking", f"{path}.$ref — цель ссылки изменена: '{base_ref}' → '{head_ref}' (не разыменовывается, трактуется консервативно)")
            )
        elif head_ref is not None:
            changes.append(
                Change("breaking", f"{path}.$ref — узел заменён ссылкой '{head_ref}' (было встроенное определение; трактуется консервативно)")
            )
        else:
            changes.append(
                Change("breaking", f"{path}.$ref — ссылка '{base_ref}' заменена встроенным определением (трактуется консервативно)")
            )

    # properties
    base_props = base.get("properties", {}) if isinstance(base.get("properties"), dict) else {}
    head_props = head.get("properties", {}) if isinstance(head.get("properties"), dict) else {}
    for name, sub in base_props.items():
        if name not in head_props:
            changes.append(Change("breaking", f"{path}.properties.{name} — свойство удалено"))
        else:
            changes.extend(classify_node(sub, head_props[name], f"{path}.{name}"))
    for name in head_props:
        if name not in base_props:
            if name in (head.get("required") or []):
                changes.append(
                    Change("breaking", f"{path}.properties.{name} — новое ОБЯЗАТЕЛЬНОЕ свойство")
                )
            else:
                changes.append(Change("additive", f"{path}.properties.{name} — новое опциональное свойство"))

    # required: поле, СУЩЕСТВОВАВШЕЕ как опциональное, стало обязательным
    base_req = set(base.get("required") or [])
    head_req = set(head.get("required") or [])
    for name in (head_req - base_req) & set(base_props.keys()):
        changes.append(
            Change("breaking", f"{path}.required — '{name}' было опционально, стало ОБЯЗАТЕЛЬНЫМ")
        )

    # enum (сужение/расширение допустимых значений)
    if "enum" in base or "enum" in head:
        base_enum = set(map(_hashable, base.get("enum") or []))
        head_enum = set(map(_hashable, head.get("enum") or []))
        for v in base_enum - head_enum:
            changes.append(Change("breaking", f"{path}.enum — значение {v!r} убрано (сужение)"))
        for v in head_enum - base_enum:
            changes.append(Change("additive", f"{path}.enum — значение {v!r} добавлено"))

    # type
    if "type" in base and "type" in head and base["type"] != head["type"]:
        changes.append(
            Change("breaking", f"{path}.type — сменился {base['type']!r} → {head['type']!r}")
        )

    # additionalProperties: true/отсутствует → false (сужение формы)
    base_ap = base.get("additionalProperties", True)
    head_ap = head.get("additionalProperties", True)
    if base_ap is not False and head_ap is False:
        changes.append(Change("breaking", f"{path}.additionalProperties — сужено до false"))

    # items — элемент массива (R-006 F-1: до фикса `items: {"$ref": ...}` — типовая форма
    # schemars для `Vec<T>`, напр. L2Snapshot.bids/asks — не рекурсировался вовсе, поэтому
    # смена элемента массива на несовместимый тип тоже проходила как CLASS=none).
    base_items = base.get("items") if isinstance(base.get("items"), dict) else None
    head_items = head.get("items") if isinstance(head.get("items"), dict) else None
    if base_items is not None or head_items is not None:
        changes.extend(classify_node(base_items or {}, head_items or {}, f"{path}.items"))

    # oneOf — T1 enum-варианты (Venue/MdPayload/EventKind/... — все закрытые enum'ы contracts)
    if "oneOf" in base or "oneOf" in head:
        base_variants = {}
        for v in base.get("oneOf") or []:
            k = _oneof_variant_key(v)
            if k is not None:
                base_variants[k] = v
        head_variants = {}
        for v in head.get("oneOf") or []:
            k = _oneof_variant_key(v)
            if k is not None:
                head_variants[k] = v
        for k, v in base_variants.items():
            if k not in head_variants:
                changes.append(Change("breaking", f"{path}.oneOf — вариант '{k}' удалён"))
            else:
                changes.extend(classify_node(v, head_variants[k], f"{path}.oneOf[{k}]"))
        for k in head_variants:
            if k not in base_variants:
                changes.append(Change("additive", f"{path}.oneOf — вариант '{k}' добавлен"))

    return changes


def _hashable(v: object) -> object:
    if isinstance(v, list):
        return tuple(v)
    if isinstance(v, dict):
        return tuple(sorted(v.items()))
    return v


def classify_schema_file(base_doc: dict, head_doc: dict) -> list[Change]:
    changes = classify_node(base_doc, head_doc, "$")

    base_defs = _defs(base_doc)
    head_defs = _defs(head_doc)
    for name, sub in base_defs.items():
        if name not in head_defs:
            changes.append(Change("breaking", f"definitions.{name} — тип удалён из схемы"))
        else:
            changes.extend(classify_node(sub, head_defs[name], f"definitions.{name}"))
    for name in head_defs:
        if name not in base_defs:
            changes.append(Change("additive", f"definitions.{name} — новый тип в схеме"))

    return changes


def classify_repo(base_files: dict[str, dict | None], head_files: dict[str, dict | None]) -> tuple[str, list[Change]]:
    """base_files/head_files: schema_file_name -> parsed JSON dict, либо None если файл не
    существовал на этом ref. Возвращает (overall_class, все Change)."""
    all_changes: list[Change] = []
    names = set(base_files) | set(head_files)
    for name in sorted(names):
        b = base_files.get(name)
        h = head_files.get(name)
        if b is None and h is not None:
            all_changes.append(Change("additive", f"{name} — новый файл схемы (новый T1-тип)"))
        elif b is not None and h is None:
            all_changes.append(Change("breaking", f"{name} — файл схемы удалён (T1-тип убран)"))
        elif b is not None and h is not None:
            file_changes = classify_schema_file(b, h)
            if not file_changes and json.dumps(b, sort_keys=True) != json.dumps(h, sort_keys=True):
                # R-006 F-1, часть 2 (ложное утверждение в выводе): классификатор нигде не
                # сравнивал СЫРОЕ содержимое файлов — "none" выводился из пустого списка
                # Change, а не из факта равенства. Если JSON-содержимое файла реально
                # отличается, а НИ ОДНО известное правило classify_node/classify_schema_file
                # не сработало — это пробел в правилах классификатора, а не "схема не
                # изменилась". Консервативно помечаем BREAKING (fail-closed): гейт никогда
                # не должен утверждать "не изменилась" на демонстрируемо изменённом файле.
                file_changes = [
                    Change(
                        "breaking",
                        f"{name} — содержимое файла изменилось, но ни одно правило классификатора "
                        "не распознало разницу (пробел в правилах); консервативно BREAKING, "
                        "гейт не утверждает 'не изменилась' на изменённом файле (R-006 F-1)",
                    )
                ]
            all_changes.extend(file_changes)
        # b is None and h is None — невозможно (name пришло из объединения ключей)

    if any(c.cls == "breaking" for c in all_changes):
        overall = "breaking"
    elif all_changes:
        overall = "additive"
    else:
        overall = "none"
    return overall, all_changes


def _load(path: Path) -> dict | None:
    if not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: diff_contract_schema.py <base-schema-dir> <head-schema-dir>", file=sys.stderr)
        return 2
    base_dir, head_dir = Path(argv[1]), Path(argv[2])

    names = set()
    if base_dir.is_dir():
        names |= {p.name for p in base_dir.glob("*.json")}
    if head_dir.is_dir():
        names |= {p.name for p in head_dir.glob("*.json")}

    base_files = {n: _load(base_dir / n) for n in names}
    head_files = {n: _load(head_dir / n) for n in names}

    overall, changes = classify_repo(base_files, head_files)

    for c in sorted(changes, key=lambda c: (c.cls, c.where)):
        tag = "BREAKING" if c.cls == "breaking" else "additive"
        print(f"{tag:8s} {c.where}")

    print(f"CLASS={overall}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
