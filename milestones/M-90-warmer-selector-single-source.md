# M-90 — у селектора прогревателя ОДИН источник: тот же, что у сервера выдачи

**Статус:** PROPOSED — набор architect'а (спека + RED + гейт) коммитится ДО диспетчеризации
dev (`04-workflow.md` §2); критик обязателен (`gates.md` §9: новая milestone-спека).
**Ревизия, на которой сняты утверждения о коде:** `origin/main` = `009d82a`.
**Предмет:** `TD-227` (MAJOR, заведён `R-210`): прод-выдача восемь суток отвечает каждому клиенту
`not_ready` при зелёных liveness-сигналах.
**Подписи founder'а НЕ требует:** состав полос подписан (`П-014` п.4, `П-029`) и не меняется;
меняется только то, ОТКУДА прогреватель его берёт.

---

## 1. Objective

Прогреватель слепка, запущенный ТЕМ вызовом, каким его зовёт прод (cron →
`deploy/bin/gateway-checkpoint-cron.sh` → `docker compose run gateway-checkpoint …`), получает
селектор (площадка, символ, таймфрейм, полосы, окно, каденция) из ТОГО ЖЕ источника, что сервер
выдачи: host `.env` → интерполяция compose → переменные `GATEWAY_*`. Собственной копии
селектора у скрипта cron'а нет; попытка задать её строкой `CHECKPOINT_*` в окружении cron'а —
отказ с именем переменной, а не молчаливое применение.

## 2. Вред и причина — ЗАМЕР (architect, 2026-10-01, ssh + прогон кода)

| # | факт | команда |
|---|---|---|
| 1 | сервер ищет `/ckpt/ckpt-8f69809dd707e8c9.bin`, прогреватель пишет `/ckpt/ckpt-b0f1ed89ec2ec142.bin` | `R-210` (strace) + `ls -la` тома `hft-platform_gateway-ckpt` |
| 2 | эти имена — РОВНО «семь полос» и «одна полоса 0.001» при прочих осях прод-селектора | временный тест на `ckpt_path_for_pub`: `seven -> ckpt-8f69809dd707e8c9.bin`, `narrow -> ckpt-b0f1ed89ec2ec142.bin` |
| 3 | скрипт cron'а несёт СВОИ копии селектора с дефолтами (`CHECKPOINT_BANDS` → `0.001`) и подаёт их аргументами | `deploy/bin/gateway-checkpoint-cron.sh:38-46`, `ARGV=(…)` |
| 4 | `docker compose run SERVICE ARGS` заменяет `command:` целиком — `--bands=${GATEWAY_BANDS}` из compose до прод-прогревателя НЕ доходит | семантика compose; `docker-compose.yml` сервис `gateway-checkpoint` |
| 5 | семь полос жили РУЧНОЙ строкой `CHECKPOINT_BANDS=…` в `/etc/cron.d/hft-journal-retention` (`R-190`: «прописан в cron :37») | `R-190` |
| 6 | деплой ставит `deploy/cron.d/*` поверх (`deploy.yml` `install_cron`); все `/etc/cron.d/hft-*` имеют mtime 2026-09-30 23:33:43 — момент деплоя; ручной строки там нет | `stat -c '%y' /etc/cron.d/*`; `diff` с репо → идентичен |
| 7 | последняя запись «правильного» слепка — 2026-09-23 12:45:02; деплой `0f9a980` — 2026-09-23 12:45 | `ls --time-style=full-iso`; `gh run list --workflow=deploy.yml` |
| 8 | host `.env` прода несёт ровно два ключа: `GATEWAY_JWT_SECRET`, `GATEWAY_BANDS` | `cut -d= -f1 .env` |
| 9 | соседний оракул `red_checkpoint_bin_prod_argv::c3ter` судит прогреватель с argv из `command:` compose — не тем вызовом, каким его зовёт прод | `crates/gateway/tests/red_checkpoint_bin_prod_argv.rs:531-` |

**Временная мера на проде (решение founder'а 2026-10-01, исполнена architect'ом):** строка
`CHECKPOINT_BANDS=0.015,0.03,0.05,0.08,0.15,0.3,0.6` возвращена в
`/etc/cron.d/hft-journal-retention` (копия до правки — `/root/hft-journal-retention.cron.bak-2026-10-01`),
прогреватель запущен один раз вручную тем же путём cron'а. **Мера стирается первым же деплоем,
меняющим `deploy/cron.d/*` или любой путь фильтра `deploy.yml`** — до вливания `M-90` выкатка без
повторения шага возвращает отказ всем.

## 3. Инвариант поставки

| ID | инвариант | оракул |
|---|---|---|
| `I-1` | слепок, записанный прогревателем, вызванным ПРОД-путём cron'а при прод-форме host `.env`, находит сервер выдачи (`LiveReducer::resume` с селектором из окружения `gateway-serve`) | `w1` (`red_m90_warmer_cron_composition.rs`) |
| `I-2` | при `.env` без полос (оба на дефолте compose) слепок также находится — у прогревателя нет СВОЕГО дефолта | `w2` |
| `I-3` | переменная селектора `CHECKPOINT_{VENUE,SYMBOL,TIMEFRAME_MS,BANDS,WINDOW_MS}` в окружении cron'а ⇒ скрипт выходит `≠ 0`, называет переменную (stderr/лог/алерт), прогреватель НЕ зовётся | `w3` |
| `I-4` | контракт `HFT_CRON_PRINT_ARGV=1` (M-48) сохранён: печатаются `--dir`, `--ckpt-dir`, `--coverage-out`, и путь покрытия совпадает с путём ретеншена | `scripts/verify_M-48.sh` (шаги `HFT_CRON_PRINT_ARGV`) зелен |

**Форма решения — указание, не требование.** Требование — `I-1`…`I-4`. Рекомендуемая форма (её
проверил architect временной правкой, все три оракула зелены, правка возвращена): скрипт перестаёт
передавать флаги селектора (передаёт только пути, `--coverage-out` и `--cursor`); бинарь
`gateway-checkpoint` при отсутствии флага берёт значение из окружения `GATEWAY_*` ТЕМ ЖЕ способом,
каким уже берёт `GATEWAY_DEPTH_CADENCE_MS` (`crates/gateway/src/bin/gateway-checkpoint.rs:94-100`);
сервис `gateway-checkpoint` в compose объявляет в `environment:` те же
`GATEWAY_{VENUE,SYMBOL,TIMEFRAME_MS,BANDS,WINDOW_MS}: ${…:-<дефолт gateway-serve>}`, что сервис
`gateway-serve`. Вариант «runner без аргументов, `command:` compose целиком» тоже удовлетворяет
`I-1`…`I-3`, но ломает `I-4` — он отвергнут.

## 4. ЗАПРЕЩЕНО

| запрещено | почему |
|---|---|
| вписывать семь полос в `deploy/cron.d/*` или в дефолт скрипта | второй источник той же величины — ровно класс `TD-227`; `w2`/`w3` краснеют |
| менять состав полос, `GATEWAY_BANDS`, `П-014`/`П-029` | граница C |
| менять `selector_fingerprint`, `ckpt_path_for`, формат слепка, `ckpt_schema_version` | смена имени/формата ⇒ холодная пересборка ≈ 23 мин (`R-187`); предмет не в отпечатке, а в источнике |
| ломать `HFT_CRON_PRINT_ARGV` (M-48) и шов `CHECKPOINT_RUNNER` | `I-4`; шов — контракт, на котором стоит `w1` |
| менять семантику `admission::readiness` | `TD-228` — отдельный предмет |
| тихо игнорировать `CHECKPOINT_*` селектора | оператор думает, что задал полосы; `w3` |
| трогать `*/tests/**`, `scripts/verify_*.sh` | sacred, зона architect'а ⇒ `!!! SCOPE VIOLATION REQUEST !!!` |

## 5. Пределы, названные честно

1. **Интерполяция compose в оракуле воспроизведена**, а не взята из `docker compose config`
   (в CI нет докера): `${VAR:-d}` и `${VAR}`. Это модель; прод-исход предъявляет §8-гейт.
2. **Селектор сервера в оракуле собран разбором окружения `gateway-serve`**, а не вызовом
   `serve_config_from_env` (у `gateway` нет зависимости на `gateway-serve`). Предмет оракула —
   композиция источников, не разбор числа.
3. **Класс «ручная правка прод-конфига, стираемая деплоем», шире этого предмета:** `M-90`
   убирает ЕДИНСТВЕННУЮ известную величину, жившую так. Сторожа на класс нет; кандидат —
   наблюдатель расхождения `/etc/cron.d/hft-*` с `deploy/cron.d/*` ДО деплоя. Не строится здесь.
4. **`TD-228` (счётчики молчат на `not_ready` в legacy-пути) не входит** — без него следующий
   такой отказ снова не виден по приборам. Отдельный предмет, следующий по порядку.

## 6. Allowed paths

**engine-dev:** `deploy/bin/gateway-checkpoint-cron.sh` · `crates/gateway/src/bin/gateway-checkpoint.rs` ·
`docker-compose.yml` (сервис `gateway-checkpoint`: `environment:`, `command:`) · `deploy/README.md`
(описание операторских ручек прогревателя).
**architect:** `milestones/M-90-*.md` · `crates/gateway/tests/red_m90_*.rs` · `scripts/verify_M-90.sh`.
**Вне зоны:** всё прочее, в том числе `crates/gateway-serve/**`, `crates/gateway/src/lib.rs`.

## 7. §Tasks

| # | Status | задача | зона | проверка |
|---|---|---|---|---|
| 1 | ⏳ OPEN | селектор прогревателя — из окружения `GATEWAY_*` при отсутствии флага (§3) | engine-dev | `w1`, `w2`; `red_checkpoint_bin_prod_argv` зелен |
| 2 | ⏳ OPEN | скрипт cron'а без собственной копии селектора; отказ на `CHECKPOINT_*` селектора (§3, §4) | engine-dev | `w1`, `w3`; `verify_M-48.sh` зелен |
| 3 | ⏳ OPEN | compose: `environment:` сервиса `gateway-checkpoint` несёт `GATEWAY_*` селектора с дефолтами `gateway-serve` (§3) | engine-dev | `w1`, `w2` |
| 4 | ⏳ OPEN | §8-гейт: после деплоя ручная строка `CHECKPOINT_BANDS` в `/etc/cron.d` стёрта деплоем, а сервер находит слепок, который пишет cron (`ls -la` тома слепков: свежий `ckpt-8f69809dd707e8c9.bin`; WS-подписка семи полос BTCUSDT получает `snapshot`, не `not_ready`) | reviewer | `verify` SKIP-шаг + `R-NNN` |

## 8. RED-оракулы — `crates/gateway/tests/red_m90_warmer_cron_composition.rs`

| ID | что доказывает | какая мутация роняет | сегодня |
|---|---|---|---|
| `w1` | прод-путь cron → скрипт → runner-заглушка → эффективный argv по семантике `compose run` → НАСТОЯЩИЙ `gateway-checkpoint` → слепок; `resume` с селектором сервера при `.env` прода: `events_decoded == 0`; контроль с чужими полосами `> 0` | собственная копия полос у скрипта; флаги селектора из скрипта поверх env | RUNTIME-RED (`events_decoded = 300`) |
| `w2` | позитивный контроль: `.env` без полос — слепок найден | «копия с правильным значением» (`CHECKPOINT_BANDS` по умолчанию = семь полос) | зелен |
| `w3` | любая из пяти `CHECKPOINT_*` селектора ⇒ exit `≠ 0`, имя переменной в выводе, runner не позван | тихое игнорирование; применение | RUNTIME-RED (runner позван) |

**Мутационный контроль набора (architect, 2026-10-01, временные правки скрипта, возвращены,
`git status` чист):** «копия с верным значением» (дефолт `CHECKPOINT_BANDS` = семь полос) ⇒
`w1` ok, `w2` FAILED (`left: 300, right: 0`), `w3` FAILED. Рекомендуемая форма (отказ на
`CHECKPOINT_*` в начале скрипта + `${CHECKPOINT_RUNNER}` без флагов селектора) ⇒ 3 passed.
Стражи setup: прод-cron зовёт скрипт; заглушка получила `compose … gateway-checkpoint`;
прогреватель вышел 0 и записал ровно один слепок.

## 9. Acceptance — `scripts/verify_M-90.sh`

Агрегатор с FAIL-счётчиком, решение по коду возврата, финальная `VERDICT`. Шаги: `task1-3` —
`w1`…`w3` + `red_checkpoint_bin_prod_argv`; `task2` — `verify_M-48.sh`; паритет с CI —
КАЖДАЯ однострочная команда `run:` из `.github/workflows/ci.yml` исполняется в форме события
`pull_request` с базой `merge-base origin/main HEAD` (`gates.md` §3); исключения перечислены
поимённо с причиной (установка инструментов, git-плумбинг, многострочные блоки);
`check_review_fa.sh` до появления `R-NNN` по `M-90` — SKIP с причиной; `task4` — SKIP (§8).
Базовая линия снимается КРАСНОЙ.

## 10. Предъявление FA

`crates/gateway/**` своей FA-записи прогревателя не имеет; опора — `docs/fa/viz-backend.md`
`VB-I-11` (провенанс истории не меняется способом старта) и `DESIGN` §22 `PL-I-8` (инцидент-класс
без алерта — дефект; см. §5 п.4). `docs/fa/ops.md` `OPS-I-8` («жив, но не работает») — ровно класс
этого отказа.

## 11. Handoff

architect (набор) → critic (`gates.md` §9, новая спека; RAW-гейт НЕ требуется: журнал и `contracts`
не тронуты) → engine-dev (задачи 1–3) → tester → reviewer (§8-гейт, задача 4; карточка `TD-227`).

## 12. Журнал кругов

| круг | вердикт | предмет | что изменено |
|---|---|---|---|
| 0 | — | набор architect'а | спека, `red_m90_warmer_cron_composition.rs`, `verify_M-90.sh` |
