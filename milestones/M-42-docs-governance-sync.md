# M-42 — docs/governance sync (R9 + докс-дрейф, ШАГ 1b)

**Статус:** PLANNED (стаб). **Риск:** R9 HIGH + Тир B (`docs/08`). Docs-only, reviewer-бэкстоп.

## Objective
Governance-источники правды разошлись с реальностью → свежая сессия получает ложную картину:
- CT-RFC-02/03/04 STATUS: PROPOSED, хотя MERGED в прод (даты в PROJECT-STATE).
- INTG-I-1..7 / CT-I-5 заявлены sacred RED в testing.md, но реальных тестов 0 (order-путь/Python не построены)
  → читатель думает, что защита действует.
- `docs/fa/README.md`, `SESSION-HANDOFF.md` устарели на десятки milestone.
- `docs/06 §2` объёмы опровергнуты замером (~10-28× занижены) — фикс на ветке `docs/06-volume-truth`, НЕ в main.

## Allowed paths (всё docs/process — architect)
- `docs/05-contract-layer.md` (RFC-статусы) · `docs/fa/README.md` · `docs/SESSION-HANDOFF.md` · `.claude/rules/testing.md` (пометка PENDING) · `docs/03-integration-contract.md` · домерж `docs/06-volume-truth`.

## Задачи
1. RFC-статусы PROPOSED→ADOPTED (с датами merge).
2. Пометить INTG-I-*/CT-I-5 как **«PENDING P3 — оракул не написан»** явно (чтобы отсутствие теста было видимым, не подразумеваемо-закрытым).
3. Обновить README/SESSION-HANDOFF под текущее состояние (M-37 done, M-38 план).
4. Домержить `docs/06-volume-truth` (фикс объёмов §2) в main.
## Гейты: reviewer (docs). critic не нужен (docs-only).
## Cross-ref: docs/08 R9 + Тир B.
