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
  ВСЕ четыре бинаря: `recorder` (ENTRYPOINT), `journal-retention` (ops-сервис
  ретеншена), режим `--mode compact` того же бинаря (компакция закрытых
  сегментов, D-COMP-3), и `gateway-serve` (M-28 WS-транспорт кокпита).
- **Compose**: `docker-compose.yml`. Сервисы:
  - `recorder` (24/7, default profile) — сбор;
  - `gateway-serve` (24/7, default profile, M-28) — WS-транспорт кокпита,
    read-only к журналу, stateless JWT-verify, БЕЗ user-БД;
  - `journal-retention` (ops profile) — выгрузка+prune;
  - `journal-compaction` (ops profile, D-COMP-3) — компакция закрытых сегментов.
  Никаких side-car'ов: всё одна кодовая база, один образ, один бинарь-портёр.
- **Планировщик**: `deploy/cron.d/journal-retention` (D4). Содержит ДВЕ
  cron-строки (разное расписание): ретеншен dry-run ежедневно + компакция.
  Только dry-run для ретеншена; apply — вручную.
- **Cron НЕ знает про apply** — это конструктивный барьер против «ретеншен
  удалил единственную копию, пока оператор спал» (класс TD-020).
- **Компакция безопасна по дизайну** (D-COMP-2): оригинал удаляется ТОЛЬКО
  после sha256-сверки сжатого `.zst`; битая копия → `.zst` удаляется,
  оригинал остаётся ГОРЯЧИМ (`Err`, exit 2). Можно запускать без dry-run'а
  по расписанию, но первый ручной прогон всё равно рекомендуется (sanity).

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
в `ssh://u659392-sub1@u659392-sub1.your-storagebox.de:23/journal/`. Файлы
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
> prune не происходит** (ColdCopyProof). Но порядок действий всё равно требует
> ручного глаза: оператор ОБЯЗАН прочитать dry-run отчёт ДО apply.

```bash
# Шаг 4.1: проверка, что dry-run отработал без сбоев.
docker compose --profile ops run --rm journal-retention
echo "exit=$?"  # должно быть 0

# Шаг 4.2: первый apply вручную (НЕ через cron).
#
# ⚠️ ЛОВУШКА TD-024: `docker compose run <svc> --mode apply` НЕ дописывает `--mode apply`
# к `command:`-блоку сервиса, а ЗАМЕНЯЕТ его ЦЕЛИКОМ. В итоге бинарь запускается с ОДНИМ
# аргументом `--mode=apply` → DEFAULT_DIR становится `./journal-data` (а не боевой
# `/journal`), DEFAULT_COLD=`./journal-cold` (а не `/cold`) → apply «отработает» на
# пустом/чужом каталоге, что для ретеншена = «нечего удалять» (на сухую ничего не
# сломается, но и данные не уйдут в Storage Box; оператор думает, что apply прошёл).
#
# Безопасная форма — повторить ВЕСЬ argv из compose `command:` с заменой `--mode=dry-run`
# на `--mode=apply`. Это гарантирует, что `--dir=/journal`, `--cold=/cold`,
# `--retain-days`, `--keep-min`, `--min-free-gb` НЕ потеряны и apply попадёт ровно в
# боевой каталог. Альтернатива через `--env`/override-файл работает, но ручное повторение
# argv — проще проверить глазами (см. §5: «команда не из репо — потенциальный drift»).
docker compose --profile ops run --rm journal-retention \
  --dir=/journal \
  --cold=/cold \
  --retain-days=14 \
  --keep-min=4 \
  --min-free-gb=10 \
  --mode=apply
echo "exit=$?"
```

Возможные exit-коды:

| Код | Значение | Реакция |
|---|---|---|
| 0 | Применили (или dry-run отработал) | — |
| 1 | Ошибка аргументов или план не построен | Чинить вызов (env, монтирование, синтаксис). |
| 2 | **Сверка холодной копии не прошла** (нет `/cold`/нет прав/битая копия) | P0: холодное хранилище сломано. Сегменты **остались горячими** (fail-closed). Проверить Storage Box, креденшелы, mount. |
| 3 | **disk_pressure** — места мало, а выгружать нечего (план пустой или всё защищено keep_min/active) | P0: данные скоро некуда писать. Возможные причины: too_young (retain_days велик), keep_min_segments велик, нет ротации (TD-006). Уменьшить keep_min или увеличить диск. |

После стабильного apply вручную (несколько дней подряд — exit 0, отчёт без `failed`)
можно доверить его cron'у, **поменяв строку в `/etc/cron.d/hft-journal-retention`**:
`--mode=dry-run` → `--mode=apply`. Это **не автоматизировано** в репо именно потому,
что «переключатель apply» — ручное решение с глазами.

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
