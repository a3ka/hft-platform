# Runbook: ретеншен журнала в проде (M-08 rev 7, задача 14, TD-020 доставка)

> «Cargo test GREEN» ≠ «функция существует в проде». Бинарь `journal-retention`
> написан, R1-R7 GREEN — но без монтирования холодного хранилища и без cron'а он
> физически не запустится. Этот документ — **руководство оператора**, без него
> ретеншен = TD-020 (оператор без доставки). Все шаги — явные, ничего «по
> конвенции» или «как обычно».

---

## 0. Что есть

- **Образ**: `hft-platform-recorder:local` (Dockerfile, multi-stage). Содержит
  ОБА бинаря: `recorder` (ENTRYPOINT) и `journal-retention` (ops-сервис).
- **Compose**: `docker-compose.yml`. Сервис `recorder` (24/7), сервис
  `journal-retention` под `profiles: ["ops"]` (запускается ТОЛЬКО явно).
- **Планировщик**: `deploy/cron.d/journal-retention` (D4). Только dry-run; apply
  — вручную.
- **Cron НЕ знает про apply** — это конструктивный барьер против «ретеншен
  удалил единственную копию, пока оператор спал» (класс TD-020).

## 1. Монтирование Hetzner Storage Box (CIFS, /etc/fstab)

> Storage Box даёт CIFS-шару. Аккаунт/креды — у founder'а/инфраструктуры
> (см. §E диспетча: «Без Storage Box apply невозможен — fail-closed»).

Создать файл `/etc/samba/credentials-hetzner` (mode 0600, root:root):

```
username=u123456-sub1
password=<от founder'а>
```

Добавить в `/etc/fstab`:

```
//u123456-sub1.your-storagebox.de/backup /mnt/journal-cold cifs credentials=/etc/samba/credentials-hetzner,uid=root,gid=root,iocharset=utf8,vers=3.0,_netdev,x-systemd.automount,x-systemd.requires=network-online.target 0 0
```

`x-systemd.automount` — Storage Box монтируется по требованию, не блокирует загрузку
хоста; `x-systemd.requires=network-online.target` — ждёт сеть. Подробнее по опциям —
Hetzner Storage Box docs (CIFS/SMBv3).

Применить:

```bash
sudo systemctl daemon-reload
sudo systemctl start mnt-journal-cold.automount
sudo mount -a
ls -la /mnt/journal-cold   # должна быть пустая (или ваша структура каталогов)
```

## 2. Установка cron-юнита

Cron-запись вызывает **тело задания** `deploy/bin/journal-retention-cron.sh` ОДНОЙ строкой
(cron не понимает переносов `\` — многострочная команда не устанавливается вовсе, см. шапку
`deploy/cron.d/journal-retention`). Поэтому ставятся ДВА артефакта: скрипт и cron-файл.

```bash
# 0) СНАЧАЛА проверить, что cron вообще примет файл (ровно это делает гейт D5):
crontab -n deploy/cron.d/journal-retention     # обязан быть exit=0

sudo install -d -o root -g root /var/log/hft /var/lib/hft
sudo install -m 0755 deploy/bin/journal-retention-cron.sh /root/hft-platform/deploy/bin/journal-retention-cron.sh
sudo install -m 0644 deploy/cron.d/journal-retention /etc/cron.d/hft-journal-retention
sudo systemctl restart cron   # или crond — зависит от дистрибутива
```

Проверка (не «файл лежит», а «оно работает» — урок TD-020):

```bash
cat /etc/cron.d/hft-journal-retention
ls -l /root/hft-platform/deploy/bin/journal-retention-cron.sh   # обязан быть исполняемым

# Прогнать задание РУКАМИ (оно dry-run по умолчанию — ничего не удалит):
sudo /root/hft-platform/deploy/bin/journal-retention-cron.sh; echo "exit=$?"
sudo tail -20 /var/log/hft/journal-retention.log

# Алерт-маркер: появляется при exit≠0 (2 = сверка холодной копии не прошла, 3 = disk_pressure),
# гаснет на успешном прогоне. Именно его пингует внешний монитор.
ls -l /var/lib/hft/retention.alert 2>/dev/null || echo "маркера нет — последний прогон успешен"
```

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
docker compose --profile ops run --rm journal-retention --mode apply
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

## 5. Что делать при алерте

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
