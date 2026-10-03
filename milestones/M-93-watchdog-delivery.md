# M-93 — сторож `ops-watchdog` доставляется деплоем, а не собирается руками

**Статус:** PROPOSED — набор architect'а (спека + RED + гейт) коммитится ДО диспетчеризации dev
(`04-workflow.md` §2); критик обязателен (`gates.md` §9: новая milestone-спека).
**Ревизия, на которой сняты утверждения о коде:** `origin/main` = `81132e1`.
**Предмет:** `TD-231` (MAJOR) и остаток `TD-220`; ROADMAP Блок 1 п.3/п.4, 9-A-bis.
**Подписи founder'а НЕ требует:** меняется способ доставки уже построенного бинаря; состав
тревог, пороги и канал (токены `П-003`) не меняются.

---

## 1. Objective

Бинарь `ops-watchdog` собирается В ПРОД-ОБРАЗЕ вместе с остальными; деплой после успешного
health-гейта достаёт его из образа РАБОТАЮЩЕГО контейнера `hft-recorder` и атомарно кладёт на хост
в постоянный путь `/usr/local/lib/hft/ops-watchdog`; cron (`deploy/cron.d/watchdog` →
`scripts/watchdog_cron.sh`) зовёт этот путь. Бинарь переживает переустановку сервера и обновляется
с каждым деплоем кода, как все остальные.

**Почему сторож остаётся ХОСТОВЫМ процессом, а не контейнером:** он опрашивает `docker ps` /
`docker inspect` и сообщает о смерти сервисов; сторож, живущий внутри docker, не сообщит о смерти
самого docker (`DESIGN` §23.1: канал тревоги не делит судьбу с наблюдаемым). Из образа берётся только
ФАЙЛ; в рантайме сторож от docker не зависит дальше, чем сегодня.

## 2. Вред и причина — ЗАМЕР (architect, 2026-10-03, ssh + чтение кода)

| # | факт | команда |
|---|---|---|
| 1 | cron `*/5` зовёт `/root/hft-platform/target/release/ops-watchdog`; бинарь существует, собран ВРУЧНУЮ 2026-10-02 10:41, журнал пишет «норма» | `ls -la …/target/release/ops-watchdog`; `tail /var/log/hft/watchdog.log`; `cat /etc/cron.d/hft-watchdog` |
| 2 | в образе бинаря нет: `Dockerfile:18` собирает `recorder journal-retention gateway-serve gateway-checkpoint wsprobe` | `git show origin/main:Dockerfile` |
| 3 | деплой бинарь не ставит: `deploy.yml` делает `git reset --hard`, `install_cron`, `docker compose up -d --build`; `target/` не трогается и не обновляется | `deploy.yml` шаг «Deploy via SSH» |
| 4 | обёртка: `WATCHDOG_BIN="${WATCHDOG_BIN:-${HFT_ROOT}/target/release/ops-watchdog}"`; без бинаря — `ALERT … бинарь не найден`, exit 1 | `scripts/watchdog_cron.sh` |
| 5 | хост — Ubuntu 26.04; ручной бинарь собран в `rust:1-slim` и зависит только от `libc`/`libgcc_s` — тот же builder, что у образа | `ldd`; `/etc/os-release`; `Dockerfile:10` |
| 6 | **поправка к реестру:** `TD-220`/`TD-231` и `R-216` говорят «ни одна тревога не вычисляется» — на 2026-10-02/03 это неверно: тревоги вычисляются ручным бинарём; верно другое — бинарь не воспроизводим и протухает с каждым изменением `crates/ops` | п.1 |

## 3. Инвариант поставки

| ID | инвариант | оракул |
|---|---|---|
| `I-1` | путь, куда деплой кладёт бинарь, РАВЕН пути, откуда его зовёт cron; путь абсолютный и вне `target/` | `w1` (`scripts/tests/red_m93_watchdog_delivery.sh`) |
| `I-2` | установка берёт бинарь из образа РАБОТАЮЩЕГО `hft-recorder` (`docker inspect -f '{{.Image}}'` → `docker create` → `docker cp …:/usr/local/bin/ops-watchdog`), кладёт исполняемым, временный контейнер удаляется, хвостов в каталоге назначения нет | `w2` |
| `I-3` | любой отказ (нет контейнера, отказ `cp`, пустой файл) ⇒ выход ≠ 0, ПРЕЖНИЙ бинарь цел (подмена атомарна: временный файл → `mv`), временный контейнер удалён | `w3`, `w4`, `w5` |
| `I-4` | образ собирает `ops-watchdog` и кладёт его в `/usr/local/bin/ops-watchdog` | `w6`; глубоко — `verify_delivery_M-08.sh` D9-deep (CI-джоб `delivery`, `HFT_DELIVERY_DEEP=1`) |
| `I-5` | деплой зовёт установку на ветке healthy (отказ установки — деплой красный) И на ветке отката (на хосте — бинарь того образа, что реально крутится) | `w7` |
| `I-6` | обёртка cron без бинаря по-прежнему кричит (`ALERT` в логе и файле тревоги, exit ≠ 0) | `w8` (сторож — зелёный сегодня) |

## 4. Форма — указание, не требование

Требование — `I-1`…`I-6`. Проверенная architect'ом форма (временной правкой, все 8 сценариев зелены,
правка возвращена):

- **`deploy/bin/install-watchdog.sh`** (новый): `DST="${HFT_WATCHDOG_DST:-/usr/local/lib/hft/ops-watchdog}"`,
  `CONTAINER="${HFT_WATCHDOG_CONTAINER:-hft-recorder}"`; `HFT_INSTALL_WATCHDOG_PRINT_DST=1` печатает `DST`
  и выходит 0, docker не зовёт; иначе: `inspect` → `create` → `cp` во временный `"$DST.new.$$"` →
  `rm` контейнера (и при отказе `cp`) → проверка непустоты → `chmod 0755` → `mv -f` в `DST`.
- **`Dockerfile`**: `--bin ops-watchdog` в строке сборки; `COPY --from=builder /build/target/release/ops-watchdog /usr/local/bin/ops-watchdog`.
- **`scripts/watchdog_cron.sh`**: дефолт `WATCHDOG_BIN` — `/usr/local/lib/hft/ops-watchdog`; текст алерта
  «бинарь не найден» называет новый способ доставки (деплой), а не ручной `cargo build`.
- **`.github/workflows/deploy.yml`**: после строки `=== healthy … ===` —
  `bash deploy/bin/install-watchdog.sh || { echo "=== WATCHDOG INSTALL FAILED ===" >&2; exit 1; }`;
  на ветке отката после повторного `up` — `bash deploy/bin/install-watchdog.sh || true` (откат и так
  красный; установка не должна маскировать его причину).

## 5. ЗАПРЕЩЕНО

| запрещено | почему |
|---|---|
| запускать сторожа в контейнере / через `docker run` из cron | канал тревоги не делит судьбу с docker (§1) |
| собирать бинарь на хосте при деплое (`cargo build` на VPS) | на хосте нет toolchain'а; второй способ сборки — второй источник бинаря (класс `TD-227`) |
| писать прямо в `DST` без временного файла; оставлять временный контейнер | `I-3` |
| менять логику тревог, пороги, `crates/ops/src/**` | вне предмета; доставка, а не поведение |
| трогать токены/канал (`TELEGRAM_*`) | `П-003`, founder |
| удалять ручной бинарь `target/release/ops-watchdog` кодом деплоя | он безвреден после переключения cron; уборка — решение после §8 |
| трогать `*/tests/**`, `scripts/verify_*.sh`, `scripts/tests/**` | sacred ⇒ `!!! SCOPE VIOLATION REQUEST !!!` |

## 6. Пределы, названные честно

1. **`deploy.yml` исполняется только на VPS.** `w7` судит ТЕКСТ ветвей шага деплоя; прод-исход
   (файл на месте, равен бинарю образа, cron им пользуется) предъявляет §8 (задача 4).
2. **D9-deep — исполняемость в образе, не запуск.** Запуск сторожа в CI опросил бы docker раннера и
   писал бы алерты; доказательство работы — журнал cron'а на проде (§8).
3. **Совместимость libc хоста и образа** держится тем, что builder — `rust:1-slim`, а хост новее
   (замер §2 п.5). Смена базового образа на более новый libc, чем у хоста, сломает запуск — тогда
   cron закричит `exit≠0` (`I-6`), молча это не пройдёт.
4. **Канал наружу по-прежнему выключен** (`П-003`): тревоги идут в журнал cron'а, не в Telegram.

## 7. Allowed paths

**engine-dev:** `Dockerfile` · `deploy/bin/install-watchdog.sh` (новый) · `scripts/watchdog_cron.sh`
(дефолт пути и текст алерта) · `.github/workflows/deploy.yml` (две строки шага «Deploy via SSH»,
§4) · `deploy/README.md` (как устроена доставка сторожа).
**`.github/workflows/deploy.yml` входит в зону `gates.md` §9** — правка перепроверяется независимым
Fable-агентом; вердикт reviewer'а на сильной модели, покрывший пункты (а)–(в) §9 для этого файла,
засчитывается.
**architect:** `milestones/M-93-*.md` · `scripts/tests/red_m93_watchdog_delivery.sh` ·
`scripts/tests/red_verify_M-93_ci_map.sh` · `scripts/verify_M-93.sh` · `scripts/verify_delivery_M-08.sh` (D9-deep).

## 8. §Tasks

| # | Status | задача | зона | проверка |
|---|---|---|---|---|
| 1 | ⏳ OPEN | образ собирает и копирует `ops-watchdog` (§4) | engine-dev | `w6`; D9-deep в CI |
| 2 | ⏳ OPEN | `deploy/bin/install-watchdog.sh` + дефолт пути в `watchdog_cron.sh` (§3 `I-1`…`I-3`, §4) | engine-dev | `w1`…`w5`, `w8` |
| 3 | ⏳ OPEN | `deploy.yml`: установка на ветке healthy (отказ — красный деплой) и на ветке отката (§4) | engine-dev | `w7` |
| 4 | ⏳ OPEN | §8-гейт: после деплоя `/usr/local/lib/hft/ops-watchdog` существует и побайтово равен `/usr/local/bin/ops-watchdog` образа работающего `hft-recorder` (`sha256sum` обоих); ближайший прогон cron пишет в журнал результат сторожа, а не «бинарь не найден»; деплой — ПО ДЖОБУ (`TD-230`) | reviewer | `verify` SKIP-шаг + `R-NNN` |

## 9. RED — `scripts/tests/red_m93_watchdog_delivery.sh`

| ID | что доказывает | мутант, который роняет | сегодня |
|---|---|---|---|
| `w1` | композиция путей деплой ↔ cron | cron по-прежнему зовёт `target/` | RED (скрипта нет) |
| `w2` | установка из образа работающего контейнера | — (позитив) | RED |
| `w3` | отказ `cp` ⇒ прежний цел, контейнер удалён | запись прямо в `DST`; нет `rm` при отказе | RED |
| `w4` | нет контейнера ⇒ `create` не зовётся | — | RED |
| `w5` | пустой файл ⇒ отказ | нет проверки непустоты | RED |
| `w6` | Dockerfile собирает и копирует | — | RED |
| `w7` | установка на обеих ветках деплоя | установка только на ветке healthy | RED |
| `w8` | обёртка без бинаря кричит | — | зелен (сторож) |

**Мутационный контроль набора (architect, 2026-10-03, временная эталонная форма §4, возвращена,
`git status` чист):** эталон ⇒ 8/8; «запись прямо в `DST`» ⇒ `w2`/`w3`/`w5` FAIL; «нет `rm` при отказе
`cp`» ⇒ `w3`; «нет проверки пустого» ⇒ `w5`; «cron зовёт `target/`» ⇒ `w1`; «установка только на ветке
healthy» ⇒ `w7`. Заглушка `docker` — в PATH, проверяет источник `cp` (`<cid>:/usr/local/bin/ops-watchdog`).

## 10. Acceptance — `scripts/verify_M-93.sh`

Агрегатор с FAIL-счётчиком. `task1-3` — проба; `task1` — `cargo build -p ops --bin ops-watchdog`,
`verify_delivery_M-08.sh` (мелкая форма; D9-deep — CI); `task2` — `cargo test -p ops`; паритет с CI —
временная копия точной карты `verify_M-91.sh` (до `TD-222`), её проба `red_verify_M-93_ci_map.sh`;
`task4` — SKIP (§8). Базовая линия снимается КРАСНОЙ.

## 11. Предъявление FA

`crates/ops/**` своим кодом не трогается; опора — `docs/fa/ops.md` **`OPS-I-10`** («объявлена ⟹
эмитится»: тревога, вычисляемая бинарём, которого нет в доставке, объявлена, но не исполняется),
`OPS-I-11` (тишина выдачи; на ветке `docs/td-237-ops-i-11`), `DESIGN` §23.1 п.1 / `PL-I-8`.

## 12. Handoff

architect (набор) → critic (`gates.md` §9) → engine-dev (задачи 1–3) → tester → reviewer (§8, задача 4;
карточки `TD-231`, `TD-220`; перепроверка `deploy.yml` по §9).

## 13. Журнал кругов

| круг | вердикт | предмет | что изменено |
|---|---|---|---|
| 0 | — | набор architect'а | спека, `red_m93_watchdog_delivery.sh`, `verify_M-93.sh`, D9-deep в `verify_delivery_M-08.sh`, проба карты |
