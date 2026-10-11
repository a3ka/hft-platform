<!-- GATE-META
milestone: M-98
audited_repo: a3ka/hft-platform
audited_base: a660b3a7b78b116a0a98bc7c9d9eb1e03153725c
audited_head: b2c54420ff1e79700f56c773b80347022df336c2
verdict: APPROVE
-->

# R-260 — M-98 задача 2: §8 деплой-гейт и close-out — APPROVE

**Роль:** reviewer. **Предмет:** код `M-98` в `main` — merge PR #342 `b2c54420`
(`R-259` APPROVE задачи 1). **Вердикт:** задача 2 исполнена, прод на merge-коммите, здоров;
милестоун **ЗАКРЫТ** (`PROJECT-STATE.md`, маркер `<!-- MS-STATE: M-98 CLOSED -->`).

Спека §7 п.3 честно говорит: прод-клиента у ветки SWITCH нет (`wsprobe` ходит legacy-путём),
поэтому прод-замер ПРЕДМЕТА невозможен и не изображается. §8 здесь — штатный: деплой
состоялся, контейнеры здоровы, журнал растёт, память в норме, legacy-путь выдачи не задет.

## 1. Деплой — пруф

```text
$ gh run list --workflow=deploy.yml --limit 3
completed	success	Deploy to VPS	Deploy to VPS	main	workflow_run	38182969663	2m39s	2026-10-11T23:05:41Z
completed	success	Merge pull request #342 from a3ka/feat/M-98-switch-provenance	Deploy to VPS	main	push	38181709107	22m17s	2026-10-11T22:45:50Z
completed	success	Deploy to VPS	Deploy to VPS	main	workflow_run	38083787037	11s	2026-10-10T20:26:58Z
$ gh run list --workflow=ci.yml --branch main --limit 2
completed	success	Merge pull request #342 from a3ka/feat/M-98-switch-provenance	CI	main	push	38181709202	19m49s	2026-10-11T22:45:50Z
completed	success	Merge pull request #341 from a3ka/docs/session-handoff-2026-10-10	CI	main	push	38082473199	19m52s	2026-10-10T20:07:04Z
```

## 2. Прод глазами (ssh)

```text
$ ssh … 'cd /root/hft-platform && git rev-parse --short HEAD; docker ps …; cat …/recorder.heartbeat; sleep 30; cat …'
b2c54420
hft-gateway-serve Up 28 seconds (healthy)
hft-recorder Up 33 seconds (healthy)
{"events":3641,…,"next_seq":863892078,"segment_index":1004,"ts_wall_ms":1791760104149,"writable":true}
# через 30 с:
{"events":5395,…,"next_seq":863893835,"segment_index":1004,"ts_wall_ms":1791760134149,"writable":true}
$ grep RssAnon /proc/<pid>/status    # pid — из docker top: /usr/local/bin/gateway-serve
hft-gateway-serve RssAnon:	     432 kB
hft-recorder RssAnon:	   11396 kB
$ docker stats --no-stream
hft-gateway-serve CPU=0.06% MEM=1.473MiB / 3GiB
hft-recorder CPU=1.66% MEM=12.65MiB / 4GiB
$ df -h /
/dev/sda1       150G   69G   76G  48% /
```

Прод-чекаут на merge-коммите, контейнеры пересозданы (uptime — секунды), heartbeat свежий,
`next_seq` растёт (+1 757 за 30 с). Память — `RssAnon` (`TD-021`), не `docker stats`.

## 3. Выдача после деплоя — `wsprobe` (legacy-путь, `I-2` спеки)

JWT подписан секретом из env контейнера (обход `TD-095`, как `scripts/measure_M-54_connect.sh`);
секрет в вывод не печатался.

```text
CPU_до=0.00%
latency_first_snapshot_ms=987 frames_received=2 wsprobe_exit=0
$ grep … /tmp/r260/snapshot.json
"history_start_seq":706605311
"history_truncated":true
"upto_seq":863896356
489916        # байт снимка
```

Legacy-путь отдаёт снимок и кадры; провенанс честный: история усечена ретеншеном (`M-92`),
начало сдвинулось вперёд относительно прод-замера `R-253` (699 592 812 → 706 605 311) — то
есть метка следует за журналом, а не застыла.

## 4. Close-out

- `PROJECT-STATE.md` — раздел `M-98` «ЗАКРЫТ», маркер `<!-- MS-STATE: M-98 CLOSED -->`.
- `TECH-DEBT.md` — `TD-251` закрыта, тело — в `docs/archive/TECH-DEBT-closed-2026-08-16.md`;
  заведены `TD-254` (`R-259` Н-1), `TD-255` (`A-053` п.4), `TD-256` (`M-98` §5 / `R-259` Н-3);
  счёт «В ПЛАНЕ» 76 → 78.
- `docs/ROADMAP.md` — колонка «Состояние» строки `TD-251`.
- За architect'ом (не моя зона): переезд `milestones/M-98-*.md` и `scripts/verify_M-98.sh` с
  пробами в `docs/archive/` по норме `Р-2`; колонка Status §11 спеки.
- Грепом по предмету искал: `TD-251`, `TD-219`, `M-98` в `TECH-DEBT.md` / `PROJECT-STATE.md` /
  `docs/ROADMAP.md`.
