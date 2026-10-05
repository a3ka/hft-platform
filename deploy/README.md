# Runbook: ретеншен И компакция журнала в проде (M-08 rev 9, задачи 14+16, TD-020+TD-022)

> «Cargo test GREEN» ≠ «функция существует в проде». Бинарь `journal-retention`
> написан, R1-R7 GREEN (операторский ретеншен) и 9/9 compaction GREEN — но без
> монтирования холодного хранилища, без cron'а и без compose-сервиса они
> физически не запустятся. Этот документ — **руководство оператора**,
> без него ретеншен = TD-020, а компакция = TD-022 (функция без оператора).
> Все шаги — явные, ничего «по конвенции» или «как обычно».

---

## 0. Что есть

- **Образ**: `hft-platform-recorder:local` (Dockerfile, multi-stage). Содержит
  ВСЕ бинари: `recorder` (ENTRYPOINT), `journal-retention` (ops-сервис
  ретеншена), режим `--mode compact` того же бинаря (компакция закрытых
  сегментов, D-COMP-3), `gateway-serve` (M-28 WS-транспорт кокпита),
  `gateway-checkpoint`, `wsprobe` (M-46 sidecar) и **`ops-watchdog`**
  (M-93 — хостовый сторож, см. §0a ниже).
- **Compose**: `docker-compose.yml`. Сервисы:
  - `recorder` (24/7, default profile) — сбор;
  - `gateway-serve` (24/7, default profile, M-28) — WS-транспорт кокпита,
    read-only к журналу, stateless JWT-verify, БЕЗ user-БД;
  - `journal-retention` (ops profile) — выгрузка+prune;
  - `journal-compaction` (ops profile, D-COMP-3) — компакция закрытых сегментов.
  Никаких side-car'ов: всё одна кодовая база, один образ, один бинарь-портёр.
- **Планировщик**: `deploy/cron.d/journal-retention` (D4). Содержит ДВЕ
  cron-строки (разное расписание): ретеншен dry-run ежедневно + компакция.
  Только dry-run для ретеншена; apply — через файл-переключатель
  `/var/lib/hft/retention.mode` (M-92, A-044 §5.1, П-031). Cron-окружение
  `RETENTION_MODE` декоративно — скрипт его игнорирует.
- **Cron НЕ знает про apply** — это конструктивный барьер против «ретеншен
  удалил единственную копию, пока оператор спал» (класс TD-020). Код и деплой
  НЕ создают файл-переключатель; включение — отдельное осознанное действие
  (оператор или architect по `П-031`).
- **Офсайт-копия** — Storage Box по SSH, ежечасно (`deploy/bin/journal-offsite-cron.sh`).
  Ретеншен НЕ копирует локально и НЕ удаляет на коробке — только сверяет
  локальную sha256 с суммой, посчитанной на СТОРОНЕ офсайт-копии
  (M-92, A-044 §5.1, R-180 F-1).
- **Компакция безопасна по дизайну** (D-COMP-2): оригинал удаляется ТОЛЬКО
  после sha256-сверки сжатого `.zst`; битая копия → `.zst` удаляется,
  оригинал остаётся ГОРЯЧИМ (`Err`, exit 2). Можно запускать без dry-run'а
  по расписанию, но первый ручной прогон всё равно рекомендуется (sanity).

## 0a. Доставка `ops-watchdog` (M-93, TD-231)

Сторож `ops-watchdog` — **хостовый процесс** (DESIGN §23.1: канал тревоги не делит
судьбу с docker; сторож внутри контейнера не сообщит о смерти самого `dockerd`).
Бинарь **собирается В ОБРАЗЕ** (`Dockerfile:18` — `--bin ops-watchdog`, копируется в
`/usr/local/bin/ops-watchdog`) и **доставляется деплоем** на хост: после healthy-гейта
`deploy.yml` (задача 3) вызывает `deploy/bin/install-watchdog.sh`. Скрипт читает
`docker inspect -f '{{.Image}}' hft-recorder`, поднимает временный контейнер, копирует
бинарь во временный файл рядом с целевым, удаляет контейнер, проверяет непустоту и
`mv -f` кладёт в `/usr/local/lib/hft/ops-watchdog`. На любом отказе — `exit≠0`,
прежний бинарь цел (`mv` атомарен), временный контейнер удалён, хвостов нет.

`/etc/cron.d/hft-watchdog` зовёт `scripts/watchdog_cron.sh` (cron-обёртка та же, что у
ретеншена: heartbeat + ALERT-маркер). Дефолтный путь бинаря в обёртке — ЭТОТ хост-путь
(`/usr/local/lib/hft/ops-watchdog`), так что установщик и cron читают одну
константу и не могут разойтись (`I-1` спеки M-93, проверяется оракулом `w1`).

Почему НЕ собирать на хосте: на VPS нет rust-toolchain'а, а два способа сборки
бинаря = два источника правды (класс `TD-227` — то, из-за чего когда-то разъехался
checkpoint-cron). ЕДИНСТВЕННЫЙ источник `ops-watchdog` — образ, который собирает
САМА VPS, и этот же образ реально крутится как `hft-recorder` (на ветке
healthy — новая сборка, на ветке отката — `PREV`, `I-5` спеки). Реестр/registry
не используется (`deploy.yml:3` — «без registry/ghcr, без токенов. Один VPS, один
образ — VPS сам собирает»), `branch-build.yml` гоняет только `fmt + clippy +
test`, без `docker build`. **CI НЕ доставляет прод-бинарь на VPS**: `ci.yml:37-44`
джоб `delivery` собирает прод-образ в CI (`HFT_DELIVERY_DEEP=1`, тот же
`scripts/verify_delivery_M-08.sh`), но не пушит его в registry и не передаёт
на VPS — прод-бинарь берётся из образа, который собирает
`docker compose up -d --build recorder gateway-serve` уже В ДЕПЛОЕ
(`deploy.yml:298`, первое звено условия `if`-блока задачи 3); сторож
устанавливается ПОСЛЕ healthy-гейта (`deploy.yml:305`, в `then`-ветке, только
когда обе health-проверки `hft-recorder` и `hft-gateway-serve`
(`deploy.yml:299-300`) прошли `healthy`).

### Проверка доставки (после деплоя)

```bash
# (1) бинарь на хосте лежит и не пустой:
ls -l /usr/local/lib/hft/ops-watchdog
# -rwxr-xr-x 1 root root ...  /usr/local/lib/hft/ops-watchdog

# (2) и БАЙТ-В-БАЙТ равен бинарю из образа работающего hft-recorder (то, что судит §8 M-93):
sha256sum /usr/local/lib/hft/ops-watchdog
docker exec hft-recorder sha256sum /usr/local/bin/ops-watchdog
# хеши ОБЯЗАНЫ совпасть; иначе — рассогласование образа и хоста (формула «inspect'а»)
# или чужой контейнер.

# (3) cron вызывает ТОТ ЖЕ путь (композиция I-1, оракул w1):
grep -h 'WATCHDOG_BIN' /etc/cron.d/hft-watchdog /etc/environment 2>/dev/null | grep . \
  && echo "FAIL: WATCHDOG_BIN в окружении cron'а — это разрывает композицию с дефолтом обёртки" \
  || echo "OK: WATCHDOG_BIN не задан — обёртка берёт дефолт /usr/local/lib/hft/ops-watchdog"

# (4) ближайший прогон cron'а в журнале — НЕ «бинарь не найден»:
sudo tail -5 /var/log/hft/watchdog.log
# ожидание: строка вида «[ops-watchdog] … — норма, ни одно условие не сработало»
# или запись ops-watchdog'а о найденной тревоге; НЕ «ALERT … бинарь не найден».
```

### ALERT `бинарь не найден/не исполняем` — что делать

Текст алерта в `scripts/watchdog_cron.sh` теперь прямо указывает источник:

```
ALERT ops-watchdog: бинарь не найден/не исполняем (/usr/local/lib/hft/ops-watchdog)
— доставка деплоем не состоялась; проверьте шаг install-watchdog.sh в
.github/workflows/deploy.yml и его последний прогон (deploy.yml упадёт КРАСНЫМ
на отказе установки — смотрите CI/Deploy)
```

Это НЕ ситуация «соберите `cargo build` на хосте» — toolchain'а на VPS нет, и
ручная сборка = второй источник бинаря (`TD-227`-класс). Действия оператора:

1. **Посмотреть последний прогон `deploy.yml` на GitHub** (Actions → Deploy). Ожидание:
   - либо джоб зелёный И в логах шага ОТСУТСТВУЕТ строка `=== WATCHDOG INSTALL
     FAILED ===` (тогда `install-watchdog.sh` отработал `exit 0`, бинарь на хосте
     должен быть — пункт 2; никакого позитивного маркера вроде `WATCHDOG INSTALL OK`
     ни `install-watchdog.sh`, ни `deploy.yml` не печатают, успех = тишина в этой
     строке + зелёный шаг);
   - либо джоб красный с `=== WATCHDOG INSTALL FAILED ===` в логах шага — это
     ветка `then` healthy-гейта (`deploy.yml:305`): `install-watchdog.sh` упал,
     `deploy.yml` напечатал маркер И `exit 1` одной строкой (`{ echo "===
     WATCHDOG INSTALL FAILED ===" >&2; exit 1; }` в правой части `||`); см.
     следующий пункт о том, где этот лог смотреть;
   - либо джоб красный БЕЗ `=== WATCHDOG INSTALL FAILED ===` в логах — это ветка
     отката `else` (`deploy.yml:308-316`): healthy-гейт не прошёл, `deploy.yml`
     уже напечатал `=== DEPLOY FAILED — logs + rollback to … ===`
     (`deploy.yml:308`), снял `docker logs …`, откатил `git reset --hard "$PREV"`
     (`deploy.yml:311`), поднял `hft-recorder`+`hft-gateway-serve` откатного
     образа (`deploy.yml:312`) и попытался поставить сторож того же образа через
     `bash deploy/bin/install-watchdog.sh || true` (`deploy.yml:315`); отказ
     установки здесь ГЛУШИТСЯ (`|| true`), маркер `=== WATCHDOG INSTALL FAILED
     ===` НЕ печатается (он бы ввёл в заблуждение — причина красного healthy-гейт,
     а не установка), и джоб красный по `exit 1` (`deploy.yml:316`) в `else`-ветке.
     В этом исходе ищите в логах `grep -F '=== DEPLOY FAILED'` (по
     префиксу, как печатает `deploy.yml:308` — `=== DEPLOY FAILED — logs +
     rollback to <sha> ===`); прежде чем разбирать установку, проверьте
     п.2.
2. **Лог шага — НЕ `journalctl`, а лог джоба GitHub Actions.** Деплой не
   systemd-сервис, юнита `journalctl -u deploy` не существует и не появится.
   Шаг деплоя — `Deploy via SSH` (`deploy.yml:239`), `uses:
   appleboy/ssh-action@v1` (`deploy.yml:240`) с `script: |` (`deploy.yml:246`)
   — это action-обёртка над SSH, а НЕ голый `ssh … 'bash -s …'`; её
   stdout/stderr — лог Actions-шага. Смотреть так: `gh run view <id> --log`
   (где `<id>` — идентификатор прогона из `gh run list`) или UI GitHub →
   Actions → Deploy → раскрыть шаг. Что реально видно при отказе
   `install-watchdog.sh`: `install-watchdog.sh` стартует с `set -euo pipefail`
   (`install-watchdog.sh:43`), **`set -x` НЕ включён** — `bash -x` trace'а нет;
   сам скрипт на отказах НЕ печатает ничего — `install-watchdog.sh:78`,
   `:83`, `:88-89`, `:100-101` это голый `exit 1` без `echo`, — и stderr даёт
   та команда, которая упала: docker CLI на `inspect` (`install-watchdog.sh:78`)
   / `create` (`:83`) / `cp` (`:88-89`), coreutils на `mkdir -p` (`:57`),
   `chmod` (`:105`), `mv -f` (`:106`); EXIT-trap (`:65-73`) молчит — `docker
   rm` и `rm -f` уведены в `/dev/null`;
   и, наконец, ветка «пустой файл» распознаётся по-разному в зависимости от
   ветки `deploy.yml`. **Ветка `then` (`deploy.yml:305`):** между строкой
   `=== healthy (recorder + gateway-serve) — deployed … ===` (`deploy.yml:301`)
   и `=== WATCHDOG INSTALL FAILED ===` (`deploy.yml:305`) в логе стоит вывод
   установщика; пусто между ними ⇒ «пустой файл» (`install-watchdog.sh:100-101`):
   все остальные точки отказа печатают stderr (п.2 выше), trap молчит. Это
   единственная ветка, где правило работает. **Ветка `else`
   (`deploy.yml:308-316`):** `=== DEPLOY FAILED — logs + rollback to <sha> ===`
   печатается ПЕРВЫМ (`deploy.yml:308`); далее два `docker logs`
   (`deploy.yml:309-310`), откат (`deploy.yml:311`), сборка откатного образа
   (`deploy.yml:312`), и только затем установщик (`deploy.yml:315`) с `|| true`
   — маркера после него НЕТ, следом сразу `exit 1` (`deploy.yml:316`). Успех
   установки и «пустой файл» здесь ОБА молчат ⇒ **исход установки в ветке
   отката из лога Actions не определим**; оператор идёт на хост: проверка
   доставки (1)-(2) (`:74-82`) и при расхождении — п.4 (ручной перезапуск).
3. **Проверить, что деплой РЕАЛЬНО выполнил установку** (а не пропустил по фильтру
   `paths:`). Фильтр `deploy.yml` (`A-046` E3) НЕ содержит `deploy/bin/**` и
   `scripts/watchdog_cron.sh`: правка одного установщика или обёртки едет на VPS только
   со следующим КОДОВЫМ push'ем или `workflow_dispatch`. Если алерт появился после
   правки ТОЛЬКО этих файлов без другого кодового коммита — это и есть причина
   (нужен `workflow_dispatch` или push в кодовый путь фильтра).
4. **Перезапустить установку вручную** (если деплой был, но бинаря нет — например,
   `/usr/local/lib/hft` удалили):

   ```bash
   # от root'а на VPS; deploy.yml идёт root'ом, sudo не нужно
   /root/hft-platform/deploy/bin/install-watchdog.sh; echo "exit=$?"
   # ожидание: exit=0, ls -l /usr/local/lib/hft/ops-watchdog показывает свежий файл
   ```
5. **Если деплой не запускался давно** (бинарь протух с прошлой правки `crates/ops/**`)
   — это НОРМАЛЬНО: `ops-watchdog` обновляется с каждым деплоем кода, как все
   остальные бинари образа. Свежесть файла на хосте = свежесть образа; рассогласование
   ловится шагом (2) выше.

Зачистка маркера тревоги (после починки):

```bash
sudo rm -f /var/lib/hft/watchdog.alert
# следующий успешный прогон cron'а не поднимет его заново (см. scripts/watchdog_cron.sh)
```

## 1. Доступ к Hetzner Storage Box через SSH-субаккаунт (НЕ CIFS)

> Storage Box этой коробки работает ТОЛЬКО через SSH (порт 23) — SMB/CIFS,
> WebDAV и External Reachability **намеренно ВЫКЛЮЧЕНЫ** в панели Hetzner'а
> (конфигурация безопасности, документированная в `docs/PENDING-SIGNATURE.md`
> и в переписке с провайдером). Никакого `/etc/fstab` и `/mnt/journal-cold`
> на проде нет и быть не может: протокол отключён, монтирование НЕ сработает.

### 1.1. Параметры доступа (уже работают, проверено 29.08.2026)

| Параметр | Значение |
|---|---|
| Цель (host) | `u659392-sub1.your-storagebox.de` |
| Порт | `23` (SSH-сервер Storage Box'а) |
| Логин | `u659392-sub1` (субаккаунт, домашний каталог — корень всех данных) |
| Ключ | `/root/.ssh/storagebox` (mode `0600`, root:root) |
| Каталог хранения | `journal/` (внутри домашнего каталога субаккаунта) |

Ключ создаётся один раз инфраструктурой (`ssh-keygen -t ed25519 -f
/root/.ssh/storagebox`), публичная часть прописывается в панели Storage Box'а
в настройках субаккаунта. Пароль субаккаунта для SSH НЕ используется —
аутентификация строго по ключу. На проде это уже сделано.

### 1.2. Проверка доступа (ручная; запускать от root'а)

```bash
# Должна сработать без запроса пароля (BatchMode + IdentitiesOnly).
# ConnectTimeout=10 — не висеть на мёртвом канале.
ssh -i /root/.ssh/storagebox \
    -o IdentitiesOnly=yes \
    -o BatchMode=yes \
    -o ConnectTimeout=10 \
    -p 23 \
    -o StrictHostKeyChecking=accept-new \
    u659392-sub1@u659392-sub1.your-storagebox.de \
    'pwd; ls -la'
```

Ожидаемый вывод: путь `~` (домашний каталог субаккаунта) и содержимое
корня — пусто или уже существующие файлы (на проде там `journal/` с
офсайт-копией).

Если `ssh` запрашивает пароль — ключ не подходит, проверяй:

1. Существует ли файл `/root/.ssh/storagebox` и его права ровно `600`
   (`stat -c '%a' /root/.ssh/storagebox` ⇒ `600`; иначе ssh откажет).
2. Публичная часть ключа совпадает с тем, что прописан в панели Storage Box'а
   (раздел «SSH-ключи» настроек субаккаунта).
3. Субаккаунт существует и активен (не удалён в панели).

### 1.3. Где живут данные

Домашний каталог субаккаунта (`pwd` в ssh-сессии выше) — это и есть корень
всего Storage Box'а для данного субаккаунта. Внутри:

* `journal/` — офсайт-копия журнала прод-хоста (см. §1.4);
* `backup/` — другие данные (если когда-либо появятся; сейчас пусто).

Никаких «монтирований», никаких CIFS-шар, никаких `x-systemd.automount`.
Все обращения — через SSH (`ssh`, `scp`, `rsync ... -e "ssh ..."`).

### 1.4. Офсайт-копия журнала (П-023)

С 2026-08-29 офсайт-копия журнала делается по расписанию (`cron.d/journal-offsite`,
раз в час, `deploy/bin/journal-offsite-cron.sh` — см. §0/§3): инкрементальный
`rsync` локального `/var/lib/docker/volumes/hft-platform_journal-data/_data/`
в `u659392-sub1@u659392-sub1.your-storagebox.de:journal/` (форма
`user@host:path`, порт 23 указан через `-e "ssh -p 23 …"`; `ssh://URL`
ЗАПРЕЩЕНА — пара с `-e` даёт «ssh ssh://…», rsync 3.4.1, R-151 Б-3 и
commit `cf686af`). Файлы
копируются ТОЛЬКО если их `mtime ≥ 15 минут` (активный сегмент пишется
прямо сейчас — копировать его = обрывок, выглядящий как целый); `recorder.heartbeat`
исключён явно (страховка от регрессии в recorder'е). Без `--delete`
(единственная защита от «команда создания бэкапа = команда уничтожения
бэкапа»). Полоса `--bwlimit=40M` (40 МБ/с из замерных 66 МБ/с канала)
через `nice -n 10 ionice -c2 -n7` — recorder 24/7 не должен голодать.
`flock -n` исключает наложение прогонов.

> **ПРИМЕЧАНИЕ про retention/cold.** Бинарь `journal-retention` исторически
> ожидает путь `/mnt/journal-cold` как `--cold` (CIFS-монтирование в старой
> конфигурации). На текущем проде, с ВЫКЛЮЧЕННЫМ SMB, retention работает в
> режиме `dry-run` (`RETENTION_MODE=dry-run` в `deploy/cron.d/journal-retention`)
> и не пишет в cold. **Перевод retention на SSH-путь — отдельная задача**;
> этот runbook не описывает её и не меняет retention-cron, чтобы не выйти
> за рамки правки §1.

## 2. Установка cron-юнита

### Модель активации (governance — почему не через `deploy.yml`)

Артефакты cron (скрипты + `cron.d`-файл) **доставляются** через репозиторий/образ, но
`install /etc/cron.d/...` — **ОСОЗНАННЫЙ РУЧНОЙ ШАГ с подписью founder ★**, а НЕ авто-действие
`deploy.yml`. Причина: активация ставит на прод **расписание, которое МОДИФИЦИРУЕТ данные**
(компакция создаёт `.zst` и удаляет `.jrnl` после сверки; ретеншен-apply удаляет выгруженные
сегменты). Цена ошибки на автомате (CI молча включил data-модифицирующий cron) выше стоимости
одного ручного шага. Компакция безопасна по дизайну (D-COMP-4 отвергает legacy/foreign;
`keep_raw` бережёт свежие; C2 — активный сегмент не трогается; §8 доказал на боевом legacy-0),
но «CI сам включил» — не то, что должно случаться без ведома founder'а.

**Порядок активации:**
1. founder ★ подтверждает активацию (какой cron: компакция сразу — безопасна и двигает дедлайн;
   ретеншен-apply — только после Storage Box, см. §1/§4).
2. Оператор ставит артефакты (команды ниже).
3. **Eyes-on ПЕРВОГО АВТО-прогона** (не ручного): дождаться времени по расписанию, проверить,
   что задание отработало САМО — свежий `*.last-success` (см. §5), `.zst` появились, legacy-0
   байт-в-байт цел (`sha256`), диск освободился, recorder не задет. Пока первый авто-прогон не
   подтверждён глазами — активация не считается завершённой (урок §8: «установлено» ≠ «работает»).

Cron-запись вызывает **тело задания** `deploy/bin/journal-retention-cron.sh` ОДНОЙ строкой
(cron не понимает переносов `\` — многострочная команда не устанавливается вовсе, см. шапку
`deploy/cron.d/journal-retention`). Поэтому ставятся ТРИ артефакта: скрипты ретеншена/компакции
И cron-файл с обеими cron-строками.

```bash
# 0) СНАЧАЛА проверить, что cron вообще примет файл (ровно это делает гейт D5):
crontab -n deploy/cron.d/journal-retention     # обязан быть exit=0

sudo install -d -o root -g root /var/log/hft /var/lib/hft
sudo install -m 0755 deploy/bin/journal-retention-cron.sh /root/hft-platform/deploy/bin/journal-retention-cron.sh
sudo install -m 0755 deploy/bin/journal-compaction-cron.sh /root/hft-platform/deploy/bin/journal-compaction-cron.sh
sudo install -m 0644 deploy/cron.d/journal-retention /etc/cron.d/hft-journal-retention
sudo systemctl restart cron   # или crond — зависит от дистрибутива
```

Проверка (не «файл лежит», а «оно работает» — урок TD-020):

```bash
cat /etc/cron.d/hft-journal-retention
ls -l /root/hft-platform/deploy/bin/journal-retention-cron.sh   # обязан быть исполняемым
ls -l /root/hft-platform/deploy/bin/journal-compaction-cron.sh   # обязан быть исполняемым

# Прогнать задание РУКАМИ (оно dry-run по умолчанию — ничего не удалит):
sudo /root/hft-platform/deploy/bin/journal-retention-cron.sh; echo "exit=$?"
sudo tail -20 /var/log/hft/journal-retention.log

# Алерт-маркер ретеншена: появляется при exit≠0 (2 = сверка холодной копии не прошла,
# 3 = disk_pressure), гаснет на успешном прогоне. Именно его пингует внешний монитор.
ls -l /var/lib/hft/retention.alert 2>/dev/null || echo "маркера нет — последний прогон успешен"

# Прогнать компакцию РУКАМИ (по дизайну безопасна — всё равно ручной sanity-check).
sudo /root/hft-platform/deploy/bin/journal-compaction-cron.sh; echo "exit=$?"
sudo tail -20 /var/log/hft/journal-compaction.log
ls -l /var/lib/hft/compaction.alert 2>/dev/null || echo "маркера нет — последний прогон успешен"
```

## 2b. Отдельный cron для компакции (D-COMP-3)

В том же `deploy/cron.d/journal-retention` есть вторая строка:

```
50 3 * * * root /root/hft-platform/deploy/bin/journal-compaction-cron.sh
```

Запускается за 17 минут ДО ретеншена (04:07 UTC), чтобы fsync-волна компакции
не наложилась на ретеншен-выгрузку. Аргументы задания --dir, --keep-raw, --mode
compact — все через env (`COMPACTION_KEEP_RAW=2`). Тот же шов гейта, что и у
ретеншена (`RETENTION_PRINT_ARGV=1` печатает argv ДО side-эффектов, парсер
бинаря проверяется НАСТОЯЩИМ, а не стабом).

## 3. Первый прогон — DRY-RUN (обязательно)

```bash
cd /root/hft-platform   # или где лежит чекаут
docker compose --profile ops run --rm journal-retention --help
```

Ожидаем: usage с дефолтами, exit 0.

Затем — собственно dry-run:

```bash
docker compose --profile ops run --rm journal-retention
```

Ожидаемый вывод (пример):

```
=== план ретеншена ===
  dir=/journal
  cold=/cold
  retain_days=14  keep_min=4  min_free_gb=10
  mode=DryRun

  offload_and_prune: 0 сегмент(ов)
  skipped: 1 сегмент(ов)
    - /journal/segment-00000000.jrnl :: active segment (writer holds it open)
  disk_pressure: нет

=== отчёт ===
  mode=DryRun
  offloaded: 0  pruned: 0  failed: 0
  freed_bytes: 0
```

Что проверить глазами (порядок важен):

1. `offload_and_prune` — пока пусто (единственный сегмент активный). Когда ротация
   накопит N+1 сегментов, в списке появятся первые кандидаты.
2. `skipped` — содержит активный сегмент С ПРИЧИНОЙ (не молча). Другие skipped —
   legacy без декларации, keep_min защита, слишком молодые.
3. `disk_pressure` — **нет** (пока диск не заполнен; если ДА — см. §5 код 3).
4. `failed` — пусто (на dry-run не заполняется).
5. `exit=0`.

Если всё ОК — теперь `/var/log/hft/journal-retention.log` пишется cron'ом ежедневно.

## 4. Переход на Apply (только после стабильного dry-run)

> Apply удаляет горячую копию. **Без полной холодной копии, сверенной по sha256,
> prune не происходит** (`ColdManifest` — суммы с УДАЛЁННОЙ стороны, M-92 §1).
> Но порядок действий всё равно требует ручного глаза: оператор ОБЯЗАН прочитать
> dry-run отчёт ДО apply.

```bash
# Шаг 4.1: проверка, что dry-run отработал без сбоев.
docker compose --profile ops run --rm journal-retention
echo "exit=$?"  # должно быть 0

# Шаг 4.2: первый apply вручную (НЕ через cron). С M-92 аргумент `--cold`
# упразднён: «скопируй-и-сверь» отвергнут R-180 F-1 как тождественная сверка.
# Скрипт cron'а (deploy/bin/journal-retention-cron.sh) сам готовит манифест
# и передаёт его бинарю через `--cold-manifest`; для apply нужен только
# файл-переключатель режима (см. §4a).
docker compose --profile ops run --rm journal-retention --mode=apply
echo "exit=$?"
```

Возможные exit-коды:

| Код | Значение | Реакция |
|---|---|---|
| 0 | Применили (или dry-run отработал) | — |
| 1 | Ошибка аргументов или план не построен | Чинить вызов (env, монтирование, синтаксис). |
| 2 | **Сверка манифеста не прошла** (нет строки в манифесте / sha256 расходится / запись на коробке испорчена) | P0: офсайт-копия сломана или расходится с локальной. Сегменты **остались горячими** (fail-closed, I-2bis). Проверить Storage Box (`ssh … sha256sum journal/<имя>`) и §4b «несовпадение копии». |
| 3 | **disk_pressure** — места мало, а выгружать нечего (план пустой или всё защищено keep_min/active) | P0: данные скоро некуда писать. Возможные причины: too_young (retain_days велик), keep_min_segments велик, нет ротации (TD-006). Уменьшить keep_min или увеличить диск. |

## 4a. Файл-переключатель режима (`RETENTION_MODE_FILE`)

> M-92, A-044 §5.1, П-031: код и деплой НЕ создают этот файл. Управление режимом —
> **отдельное осознанное действие оператора** (или architect'а по `П-031`), и
> именно поэтому оно вынесено из cron-файла: cron перезаписывается каждым
> кодовым деплоем (2026-09-23 так была стёрта строка `CHECKPOINT_BANDS`),
> и деплой не должен иметь права случайно включить apply.

```bash
# Включить apply на следующий прогон (после §4 — стабильные dry-run'ы и проверка
# аудит-следа):
echo apply > /var/lib/hft/retention.mode

# Выключить (вернуться в dry-run) — обнулить или удалить:
: > /var/lib/hft/retention.mode
# или
rm /var/lib/hft/retention.mode
```

Что важно:

- **Содержимое файла** проверяется ПОСЛЕ `tr -d '[:space:]'`: ровно `apply` ⇒ apply,
  любой другой текст / пусто / нет файла ⇒ `dry-run`. Пробелы и переводы строк
  игнорируются.
- **Cron-задание НЕ проверяет наличие файла между итерациями** — каждый прогон
  читает файл заново. Чтобы применить новое значение, НЕ нужно перезапускать cron.
- **НЕ редактируйте `RETENTION_MODE` в `/etc/cron.d/journal-retention`** —
  скрипт cron'а эту переменную игнорирует (`read_mode` в
  `deploy/bin/journal-retention-cron.sh`). Она оставлена в cron-файле для grep-видимости
  «по cron'у не apply» при аудите deploy.yml.
- **Аудит-след в `/var/lib/hft/prune-audit/`** — три записи на прогон
  (`<UTC>-<pid>-plan.txt`, `-manifest.txt`, `-report.txt`); записи прошлых прогонов
  НЕ перезаписываются (уникальные базовые имена, A-044 O-2). После apply с
  включённым переключателем ОБЯЗАТЕЛЬНО заглянуть в отчёт — там `pruned <имя>` /
  `kept <имя> <причина>` построчно, без заголовка.

## 4b. Несовпадение копии на коробке — процедура восстановления

> M-92, A-044 §5.1, A-045 §3: испорченная или отсутствующая копия СТАРЕЙШЕГО
> кандидата удерживает ВСЁ удаление до починки (fail-closed, I-2bis, `I-2ter`).
> Это не дефект — это страховка от тихого «удалили локально, а копия была битой».
> Шаги ниже возвращают конвейер в строй; следующий прогон удалит то, что было
> удержано, плюс всё, что накопилось за время простоя.

Типичный сценарий: в `/var/lib/hft/retention.alert` — `exit=2`, в
`/var/log/hft/journal-retention.log` — строка `FAIL /journal/segment-NNNNNN.jrnl: <reason>`,
а в `/var/lib/hft/prune-audit/<…>-report.txt` — `kept segment-NNNNNN.jrnl <причина>`.
Это значит, что бинарь СРАВНИЛ локальную sha256 с суммой из манифеста и нашёл
расхождение (или строка в манифесте отсутствует) — сегмент остался горячим,
**остальные тоже** (префикс плана обрывается на первом неподтверждённом).

```bash
# 0) Прочитать отчёт: какой именно сегмент удержан и почему.
#    Имя берём из строки `kept <имя> <причина>` в свежайшем *-report.txt
#    (файл с МАКСИМАЛЬНЫМ mtime в /var/lib/hft/prune-audit/).
SEG=$(ls -t /var/lib/hft/prune-audit/*-report.txt | head -1 \
      | xargs awk '/^kept / { print $2; exit }')
echo "удержанный сегмент: $SEG"

# 1) Убедиться, что локальный файл в порядке (если локально битый — это ДРУГАЯ
#    проблема, M-92 её не лечит, стоп и зовём architect'а).
test -f "/var/lib/hft/journal-data/.../$SEG"   # путь зависит от bind-mount
#    Скриптом `journal::list_segments` он виден через `journal-replay-digest-cron.sh`,
#    либо напрямую:
ssh -i /root/.ssh/storagebox -p 23 u659392-sub1@…your-storagebox.de \
    ls -la journal/

# 2) Сверить суммы НАПРЯМУЮ — локально и удалённо.
LOCAL=$(sha256sum "/<путь-к-журналу>/$SEG" | awk '{print $1}')
REMOTE=$(ssh -i /root/.ssh/storagebox -o IdentitiesOnly=yes -p 23 \
         u659392-sub1@…your-storagebox.de sha256sum "journal/$SEG" | awk '{print $1}')
echo "local : $LOCAL"
echo "remote: $REMOTE"
# Если local ≠ remote — копия на коробке испорчена или устарела; едем к (3).
# Если local == remote — манифест собран из старого слепка; см. (4).

# 3) ПЕРЕЗАЛИТЬ файл на коробку. ОБЯЗАТЕЛЬНО через rsync --checksum, не обычный
#    rsync: обычный rsync смотрит на размер+mtime и НЕ перезальёт файл той же
#    длины и того же mtime, даже если содержимое испорчено. --checksum сверяет
#    по sha256 и перезаписывает всегда, когда локальный и удалённый хеши
#    расходятся. ИМЯ — единственный аргумент; путь внутри Storage Box —
#    `journal/<имя>`.
rsync -av --checksum \
    -e 'ssh -i /root/.ssh/storagebox -p 23' \
    "/<путь-к-журналу>/$SEG" \
    "u659392-sub1@…your-storagebox.de:journal/$SEG"
#    rsync сам создаст ~/.ssh/known_hosts при первом запуске; если ключ
#    зашифрован паролем — `ssh-agent`.

# 4) Если local == remote (манифест собран из старого слепка коробки) — копия
#    в порядке; причина удержания в том, что `RETENTION_REMOTE_SHA_CMD` считал
#    сумму с ЗАПИСИ, которая с тех пор была перезалита, ночной офсайт-скрипт
#    (`deploy/bin/journal-offsite-cron.sh`) уже отвёз свежий файл. Достаточно
#    дождаться следующего прогона cron'а (04:07 UTC) — он прочитает СВЕЖИЙ
#    манифест, сверит с локальным, и удалит удержанный сегмент (плюс то, что
#    накопилось после него).
```

После починки **вернуть** переключатель в dry-run для следующего прогона (если
только это не был целенаправленный apply) — иначе следующий же 04:07 UTC
применит удаление, что для первого раза после ручной починки обычно
нежелательно:

```bash
: > /var/lib/hft/retention.mode    # dry-run
```

На СЛЕДУЮЩИЙ прогон (04:07 UTC или ручной) ожидание: `exit=0`, в
`*-report.txt` — `pruned <имя>` для всех ранее удержанных сегментов плюс
обычная норма удаления. Если опять `exit=2` — перезалить ещё раз (rsync
мог быть прерван) и повторить.

## 5. Мониторинг и алерты

Два независимых сигнала — оба нужны, потому что они ловят РАЗНЫЕ отказы:

**(A) `*.alert` — «прогон УПАЛ».** `/var/lib/hft/retention.alert` и
`/var/lib/hft/compaction.alert`: присутствие = последний прогон завершился с ошибкой (внутри
timestamp + exit code); успешный прогон маркер гасит.

**(B) `*.last-success` — «прогон СЛУЧИЛСЯ» (позитивный heartbeat).** `/var/lib/hft/
retention.last-success` и `compaction.last-success`: UTC-таймстамп последнего УСПЕШНОГО прогона.
Зачем отдельно от (A): отсутствие `*.alert` НЕОДНОЗНАЧНО — «всё ок» ИЛИ «cron НИКОГДА не
запускался» (не установлен / `crond` мёртв / ребут без cron). Для deadline-критичной компакции
второе — тихая катастрофа: замолчит, диск заполнится, никто не узнает по `*.alert`. Поэтому
внешний монитор (zabbix/nagios/cron-watchdog) ОБЯЗАН алертить по **СВЕЖЕСТИ** `*.last-success`:

```bash
# Порог = период расписания + запас. Оба задания суточные ⇒ старше ~26 ч = cron НЕ отработал.
now=$(date -u +%s)
for m in compaction retention; do
  f=/var/lib/hft/$m.last-success
  if [ ! -f "$f" ]; then echo "ALERT $m: heartbeat отсутствует — cron не запускался НИ РАЗУ"; continue; fi
  age=$(( now - $(date -u -d "$(cat "$f")" +%s) ))
  [ "$age" -gt 93600 ] && echo "ALERT $m: heartbeat протух (${age}s > 26h) — cron молчит"
done
```

Правило (сессионный урок, 9 дефектов класса «отсутствие не наблюдается»): **и сбой, и МОЛЧАНИЕ
обязаны быть видимы.** `*.alert` покрывает сбой; `*.last-success` + freshness-монитор — молчание.

### Разбор `*.alert` (прогон упал)

Внутри `*.alert` — timestamp + exit code.

**Ретеншен:**

```bash
cat /var/lib/hft/retention.alert
tail -50 /var/log/hft/journal-retention.log

# Это код 2 (cold verify)?
docker compose --profile ops run --rm journal-retention --help
mount | grep journal-cold
ls -la /mnt/journal-cold
docker compose --profile ops run --rm journal-retention   # dry-run с verbose

# Это код 3 (disk_pressure)?
df -h /journal
docker compose --profile ops run --rm journal-retention   # смотрим plan.disk_pressure
```

**Компакция:**

```bash
cat /var/lib/hft/compaction.alert
tail -50 /var/log/hft/journal-compaction.log

# Код 2 у компакции = sha256 .zst mismatch (D-COMP-2). Данные НЕ потеряны
# (оригинал оставлен ГОРЯЧИМ), но сам факт требует внимания:
df -h /journal
docker compose --profile ops run --rm journal-compaction   # следующий прогон сам перепишет битый .zst
ls /journal | grep '\.jrnl\.\?$'
ls /journal | grep '\.zst$'
```

**Зачистка маркера** (после починки):

```bash
rm /var/lib/hft/retention.alert /var/lib/hft/compaction.alert
```

## 5a. Компакция — отдельный runbook (D-COMP-3, rev 9)

> Компакция безопасна по дизайну, но оператор ВПРАВЕ хотеть sanity-check перед
> тем как доверить её cron'у. Первый ручной прогон — то же, что для ретеншена
> в §3, только без страха «потеряю данные».

```bash
# 1) Проверить, что бинарь принимает mode compact:
docker compose --profile ops run --rm journal-compaction --help   # usage с тремя режимами

# 2) Проверить, что на боевом каталоге работает:
docker compose --profile ops run --rm journal-compaction
echo "exit=$?"
```

Ожидаемый вывод (пример для боевого состояния с N закрытыми сегментами):

```
=== компакция закрытых сегментов (D-COMP-3) ===
  dir=/journal
  keep_raw=2  compact_level=3

  compacted: 22 сегмент(ов)
    - /journal/segment-00000000.jrnl → /journal/segment-00000000.jrnl.zst (260 MiB → 28 MiB, −89.2%)
    - /journal/segment-00000001.jrnl → /journal/segment-00000001.jrnl.zst (255 MiB → 27 MiB, −89.4%)
    ...
  итого: 5.5 GiB → 600 MiB (коэффициент 9.38×)
```

Проверить глазами:

1. `compacted` — не ноль (на боевом проде >0, на тестовой VM может быть пусто — это ОК).
2. Суммарный коэффициент сжатия — в районе 8–12× для сжатия биржевых MD-данных.
3. `exit=0`.
4. После прогона: `journal::stream` читает и активный сегмент, и `.zst` сегменты
   БЕЗ потери данных (D-COMP-1, общий хелпер dedup).

Что НЕ делать с компакцией:

- **НЕ призывать её на активный сегмент.** `compact_segment` это ОТВЕРГАЕТ,
  но и не вызывайте руками — это попытка записать v2-фреймы в zstd-поток, рецепт
  повреждения.
- **НЕ удалять `.zst` руками, не зная, что оригинал тоже удалён.** `compact_closed_segments`
  это делает сам через `ColdCopyProof`-принцип (D-COMP-2), а ручное удаление без
  знания состояния приведёт к двукратному чтению сегмента (если оригинал остался)
  или потере данных (если был только `.zst`).

`/var/lib/hft/retention.alert` — маркер-файл. Его присутствие = последний прогон
ЗАВЕРШИЛСЯ С ОШИБКОЙ. Внутри — timestamp + exit code.

Шаги:

```bash
# Что случилось?
cat /var/lib/hft/retention.alert
tail -50 /var/log/hft/journal-retention.log

# Это код 2 (cold verify)?
docker compose --profile ops run --rm journal-retention --help
mount | grep journal-cold
ls -la /mnt/journal-cold
docker compose --profile ops run --rm journal-retention   # dry-run с verbose

# Это код 3 (disk_pressure)?
df -h /journal
docker compose --profile ops run --rm journal-retention   # смотрим plan.disk_pressure

# Починили — убираем маркер.
rm /var/lib/hft/retention.alert
```

## 6. Аварийный rollback

Если после merge что-то пошло не так (recorder не пишет, новый сегмент не
открывается, сегменты теряются) — **данные дороже фичи**. На VPS:

```bash
cd /root/hft-platform
git log --oneline -5                   # запоминаем текущий SHA
git reset --hard <prev-stable-sha>     # откат
docker compose up -d --build recorder  # пересборка
```

§8 деплой-гейт (см. milestone rev 6/7) проверяет **байт-в-байт целостность
старого боевого сегмента** перед merge — это и есть страховка.

## 7. Что НЕ делать

- **НЕ включать apply в cron без ручной валидации dry-run'а** в течение ≥3 дней.
  TD-020 = «автоматизация удалила единственную копию».
- **НЕ менять `--keep-min` до 0** без архитектурного обоснования: последние N
  сегментов защищены для реплея/диагностики, не для «чтобы быстрее чистить».
- **НЕ подменять cron на systemd timer без обновления `deploy/cron.d/`**:
  планировщик В РЕПО, а не в голове оператора (ровно так TD-020 и родился).
- **НЕ менять ENTRYPOINT образа на journal-retention**: recorder 24/7, ретеншен
  — отдельный процесс; падение одного не валит другое.

## 8. Связанные документы

- `milestones/M-08-data-durability.md` — milestone rev 7, §Tasks #14, контракт
  доставки D1-D6.
- `scripts/verify_delivery_M-08.sh` — гейт доставки (D1-D6 + D1-deep).
- `docs/06-data-layer-and-storage.md` — retention/cold, требования к `/journal` и
  `/cold`.
- `crates/journal/src/bin/journal-retention.rs` — реализация CLI.

---

# Прогреватель слепка чекпоинта (M-90, TD-227)

`deploy/bin/gateway-checkpoint-cron.sh` зовётся из `deploy/cron.d/journal-retention` (`*/15`)
и через `docker compose run --rm gateway-checkpoint` снимает чекпоинт `Reducer` в том
`gateway-ckpt`, чтобы `gateway-serve` подхватил его при подключении клиента без холодного
реплея (M-38b, TD-044).

## 8a. Единый источник селектора (M-90, §3 спеки)

Прогреватель и сервер выдачи берут селектор (площадку, символ, таймфрейм, полосы, окно,
каденцию) из ОДНОГО источника: `host .env` → `docker-compose.yml environment:` сервисов
`gateway-serve` и `gateway-checkpoint` → `GATEWAY_*`. **У cron-обёртки нет СВОЕЙ копии
селектора** — это и есть лечение `TD-227` (прод-выдача восемь суток отвечала `not_ready`
при зелёных liveness-сигналах; корень — `GATEWAY_BANDS` в `.env` против `CHECKPOINT_BANDS=0.001`
в обёртке).

| Где | Что |
|---|---|
| `docker-compose.yml` (`environment:` сервиса `gateway-checkpoint`) | объявляет `GATEWAY_VENUE` / `_SYMBOL` / `_TIMEFRAME_MS` / `_BANDS` / `_WINDOW_MS` / `_DEPTH_CADENCE_MS` с ТЕМИ ЖЕ дефолтами `${VAR:-…}`, что у `gateway-serve` |
| `gateway-checkpoint` (binary) | при отсутствии флага читает `GATEWAY_*` env (тот же путь, что `M-68` задача 23 ввела для `--depth-cadence-ms`); флаг имеет приоритет как явная команда оператора |
| `deploy/bin/gateway-checkpoint-cron.sh` | НЕ передаёт флаги селектора (`--venue`/`--symbol`/`--timeframe-ms`/`--bands`/`--window-ms`/`--depth-cadence-ms`); на любую из шести `CHECKPOINT_*` селектора в своём окружении — отказ с именем переменной, runner НЕ зовётся |

## 8b. Операторские ручки cron-обёртки прогревателя

Разрешено менять в `/etc/cron.d/hft-journal-retention` (для прогревателя) — это НЕ
селектор, источник через `host .env`:

| Env | Назначение | Дефолт |
|---|---|---|
| `CHECKPOINT_RUNNER` | шов: подставить прямой бинарь (использует гейт `verify_M-48.sh`) | `docker compose run --rm gateway-checkpoint` |
| `CHECKPOINT_JOURNAL_DIR` | путь журнала (внутри контейнера `/journal`, на хосте — том) | `/journal` |
| `CHECKPOINT_CKPT_DIR` | путь чекпоинт-тома | `/ckpt` |
| `CHECKPOINT_COVERAGE_OUT` | путь артефакта покрытия (КОНТРАКТ с retention — `verify_M-48` канарейка КОМПОЗИЦИИ) | `/ckpt/covered_through_seq` |
| `CHECKPOINT_CURSOR` | `--cursor LATEST` по умолчанию; `CHECKPOINT_CURSOR=<seq>` для инкрементального прогона | `LATEST` |
| `CHECKPOINT_LOG` / `CHECKPOINT_ALERT_FILE` / `CHECKPOINT_LAST_SUCCESS` | observability (D9) | `/var/log/hft/gateway-checkpoint.log` и т.д. |

**Запрещено в окружении cron'а** (отказ с именем переменной в stderr/логе/алерте, прогреватель
не зовётся):

```text
CHECKPOINT_VENUE, CHECKPOINT_SYMBOL, CHECKPOINT_TIMEFRAME_MS,
CHECKPOINT_BANDS, CHECKPOINT_WINDOW_MS, CHECKPOINT_DEPTH_CADENCE_MS
```

Проверяется демонстрационно (после деплоя):

```bash
# Любая из шести — exit≠0, имя переменной в stderr:
CHECKPOINT_BANDS=0.001 /root/hft-platform/deploy/bin/gateway-checkpoint-cron.sh
# ALERT CHECKPOINT_BANDS в окружении cron'а ЗАПРЕЩЕНА — собственный источник селектора
# запрещён (TD-227, M-90 I-3). Источник — GATEWAY_* из host .env через compose
# environment: сервиса gateway-checkpoint.

# argv, который прогреватель ВЫПОЛНИЛ БЫ (контракт HFT_CRON_PRINT_ARGV, M-48):
HFT_CRON_PRINT_ARGV=1 /root/hft-platform/deploy/bin/gateway-checkpoint-cron.sh
# --dir
# /journal
# --ckpt-dir
# /ckpt
# --coverage-out=/ckpt/covered_through_seq
# --cursor
# LATEST
```

## 8c. Что НЕ делать (прогреватель)

- **НЕ вписывать селектор в `/etc/cron.d/hft-*` строкой `CHECKPOINT_*=…`** — это и есть
  класс `TD-227` (два источника одной величины). Все шесть имён отвергаются скриптом.
- **НЕ менять `GATEWAY_BANDS` в `docker-compose.yml`** в обход `host .env`: дефолт
  `${GATEWAY_BANDS:-0.001}` уже снимает замер R-185 §1; канонический набор
  `0.015,0.03,0.05,0.08,0.15,0.3,0.6` включается ОДНОЙ строкой в `host .env`.
- **НЕ добавлять `--bands=…` (и любой `--venue`/`--symbol`/`--timeframe-ms`/`--window-ms`)
  в `command:` compose-сервиса `gateway-checkpoint`** — единый источник с `gateway-serve`
  через `environment:` блок, откуда их читает бинарь. `--depth-cadence-ms` — исключение
  (`red_checkpoint_bin_prod_argv::c3ter` требует его присутствия в argv).

## 9. M-94 — профиль расчётов: «прогреть, потом переключить» (`П-032`)

После M-94 у `gateway-serve` и `gateway-checkpoint` **ЕДИНСТВЕННЫЙ носитель** определения
расчёта — `config/calc-profile/active.env` в репозитории (запечён в образ через `COPY` в
`Dockerfile`, объявлен compose-`environment:` как `GATEWAY_CALC_PROFILE`). Смена любой
величины, меняющей имя слепка (`GATEWAY_BANDS`, `GATEWAY_DEPTH_CADENCE_MS`,
`GATEWAY_TIMEFRAME_MS`, `GATEWAY_WINDOW_MS`, `GATEWAY_ALLOWED_PROFILES` через тройку
`timeframe/window/cadence`), идёт по схеме «прогреть, потом переключить» — иначе
`gateway-serve` после `up` будет отдавать `not_ready` каждому клиенту до тех пор, пока
`gateway-checkpoint` не пройдёт полный цикл прогрева (≈16–23 мин, замер M-84/R-187).

### 9a. Смена, НЕ меняющая имя слепка (один PR)

Величины `GATEWAY_HEATMAP_WINDOW` и `GATEWAY_VP_BIN_WIDTH_E8` в `selector_fingerprint`
НЕ участвуют (`crates/gateway/src/lib.rs:4252-4274`) — их смена не инвалидирует слепок.
Один PR:

1. Правка `config/calc-profile/active.env` (только эти 2 ключа + bump `CALC_PROFILE_VERSION`
   на +1).
2. `CALC-PROFILE-DECISION: П-NNN` в теле коммита; заголовок `## П-NNN` в
   `docs/PENDING-SIGNATURE.md` ЭТОГО коммита. Барьер `check_calc_profile.sh` в CI.
3. Merge → деплой поднимает выдачу с новым профилем. Слепок `active.env` (тот же селектор)
   остаётся, пересборка НЕ нужна.

### 9b. Смена, МЕНЯЮЩАЯ имя слепка (ДВА PR)

Величины `GATEWAY_BANDS`, `GATEWAY_DEPTH_CADENCE_MS`, `GATEWAY_TIMEFRAME_MS`,
`GATEWAY_WINDOW_MS`, `GATEWAY_ALLOWED_PROFILES` (через `timeframe/window/cadence`)
входят в `selector_fingerprint` ⇒ меняют имя файла. Два PR, ни один не переключает
выдачу сам.

#### PR «прогрев» — ДО деплоя

1. Добавить `config/calc-profile/next.env` — полный профиль с НОВЫМИ значениями
   и `CALC_PROFILE_VERSION >` версии `active.env` (барьер в CI это проверяет).
2. `CALC-PROFILE-DECISION: П-NNN` в теле коммита; заголовок `## П-NNN` в
   `docs/PENDING-SIGNATURE.md` ЭТОГО коммита.
3. Merge → деплой пройдёт штатно (`active.env` не менялся, выдача работает
   на старом профиле). cron-обёртка `gateway-checkpoint-cron.sh` в КАЖДОМ цикле
   делает дополнительный прогон:
   - прогон `active` — `compose run --rm gateway-checkpoint` (наследует
     `GATEWAY_CALC_PROFILE=/etc/hft/calc-profile/active.env` из compose-`environment:`);
   - прогон `next` — `<runner> -e GATEWAY_CALC_PROFILE=/etc/hft/calc-profile/next.env
     gateway-checkpoint ...` (только при наличии `$HFT_ROOT/config/calc-profile/next.env`).
   Оба прогона пишут слепки в КОРЕНЬ тома `gateway-ckpt` (`--ckpt-dir=/ckpt` общий);
   покрытие — в РАЗНЫЕ файлы (`/ckpt/covered_through_seq` для `active`,
   `/ckpt/covered_through_seq.next` для `next`), чтобы курсор `next` (всегда ДАЛЬШЕ
   `active` — больше хвоста) не сдвинул файл, который читает ретеншен (милестоун
   §3.5: «покрытие `next` НЕ смеет идти в путь ретеншена»). Старый слепок
   `active` продолжает освежаться (селектор тот же).

#### Приёмка прогрева (между PR «прогрев» и PR «переключение»)

- Слепок `next.env` существует и свежий:

  ```bash
  ls -la /var/lib/docker/volumes/hft-platform_gateway-ckpt/_data/
  # ckpt-<fp_old>.bin   <-- active (старый)
  # ckpt-<fp_new>.bin   <-- next (новый, имя по `ckpt_path_for_pub` селектора next.env)
  # covered_through_seq
  # covered_through_seq.next
  ```

- `<ckpt-fp>.profile` РЯДОМ с каждым слепком — отчёт о ПРОЧИТАННОМ файле профиля
  (милестоун §3.6, оракул `p6`). Если `*.profile` отсутствует рядом с `ckpt-<…>.bin` —
  прогон `gateway-checkpoint` упал, и приёмка НЕ пройдена.

- **Проверка селектора** в `next.env` — разобрать `next.env` и сравнить с селектором
  в `active.env`. Если тройка `(timeframe_ms, window_ms, depth_cadence_ms)` НЕ входит
  в `GATEWAY_ALLOWED_PROFILES` (новая политика допуска ещё не развёрнута) — слепок
  пишется, но сервер его НЕ ОБСЛУЖИВАЕТ (милестоун §3.2, связь тройки). Это
  легитимный переходный сценарий, и переключение ниже его снимает.

#### PR «переключение»

1. Содержимое `next.env` становится `active.env`, `next.env` удаляется.
2. `CALC_PROFILE_VERSION` увеличивается на +1 (от версии `next.env`).
3. `CALC-PROFILE-DECISION: П-NNN` (тот же токен подписи, что и в «прогреве»).
4. Merge → деплой проходит гейт `deploy/bin/calc-profile-gate.sh` (милестоун §3.7):
   - host `.env` БЕЗ ключей профиля (барьер `g2`/`g3`: иначе выдача переключится на
     `active.env` минуя профиль, класс TD-227);
   - слепок НОВОГО профиля (`ckpt-<fp_new>.bin`) лежит в томе
     `$CALC_GATE_CKPT_HOST_DIR` (барьер `g4`/`g5`/`g6`).
5. Тело деплоя (`deploy/bin/deploy-apply.sh "$PREV"`) переустанавливает cron, поднимает
   обе службы, ждёт `healthy`, ставит сторож. На любом отказе — откат к PREV
   (чекаут, тег образа, cron, профиль — на PREV), `up` на PREV.

### 9c. После смены профиля

- **Старый слепок НЕ удалять** вручную: `rm /ckpt/ckpt-<fp_old>.bin` оставит
  `gateway-serve` без реплея истории, а `journal-retention` в следующем цикле может
  удалить префикс журнала, нужный для отката к `active` (милестоун §5: «удаление
  слепков запрещено»). Старый слепок лежит до приёмки `next` или до следующей
  смены профиля.
- **Не дублировать профиль** в `host .env`: ровно та ошибка, против которой стоит
  гейт. Если `host .env` ещё несёт `GATEWAY_BANDS=…` от ДО-M-94 формы — это
  задача 11 (милестоун), архитектор/ревьюер снимает ПОСЛЕ merge M-94 и ДО
  следующего деплоя (окно ≈16 мин, пока CI бежит).
