# TECH-DEBT — открытый долг

> **Reviewer-owned.** Открытые долги/риски, замеченные при работе. Закрытые переносятся вниз.

## OPEN
- **TD-001** recorder Docker-образ работает root'ом (M-00 заглушка). Hardening (non-root +
  права journal-тома) — при реальном recorder (M-01). Severity: MINOR.
- **TD-002** `hetzner-server` приватный ключ был вставлен в чат (скомпрометирован). Пересоздать
  на лэптопе + заменить на VPS при случае. Severity: MINOR (доступ и так только founder+ключи).
- **TD-003** `[verify-at-impl]` по Hyperliquid и Binance (WS-эндпоинты, форматы, rate-лимиты,
  глубина L2, подпись действий) — уточнить при реализации venues. Severity: NOTE.

## CLOSED
- (пусто)
