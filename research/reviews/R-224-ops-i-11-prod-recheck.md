<!-- GATE-META
milestone: TD-237
audited_repo: a3ka/hft-platform
audited_base: 74bd371ef5eb85d1adfd0d2ba88c7e02821ded26
audited_head: bd7c84c471864de9cbabc2ba342e221b563de45e
verdict: REJECT
-->

# R-224 — перепроверка `gates.md` §9: `docs/fa/ops.md` OPS-I-11 «исполнение на проде» (ветка `docs/ops-i-11-prod`)

**Роль:** independent recheck (Fable, свежий контекст; автор правки — ведущий architect, я его не продолжаю).
**Предмет:** ветка `origin/docs/ops-i-11-prod`, вершина взята командой (`git rev-parse origin/docs/ops-i-11-prod` → `bd7c84c…`), база — `git merge-base origin/main HEAD` → `74bd371…`. Диф — одна строка `docs/fa/ops.md:479` (строка OPS-I-11 таблицы §6).
**Вердикт: REJECT** — две из пяти проверенных подстрок правки не выдерживают проверки командой (`gates.md` §9 п. (а)). Обе лечатся правкой той же одной строки; конструкция (что именно изменилось в проде и где предел) верна.

## §0 — Вершина и дерево

```
$ git fetch origin && git rev-parse origin/docs/ops-i-11-prod
bd7c84c471864de9cbabc2ba342e221b563de45e
$ git merge-base origin/main origin/docs/ops-i-11-prod
74bd371ef5eb85d1adfd0d2ba88c7e02821ded26
$ git log --oneline origin/main..origin/docs/ops-i-11-prod
bd7c84c docs(fa/ops): OPS-I-11 — исполнение на проде с M-93 (бинарь доставляется деплоем), предел — канал П-003 [architect]
$ git worktree add --detach /tmp/hft-recheck-ops11 origin/docs/ops-i-11-prod
$ git diff 74bd371..HEAD --stat
 docs/fa/ops.md | 2 +-
$ git diff --stat HEAD origin/main -- Dockerfile deploy/ .github/workflows/deploy.yml scripts/watchdog_cron.sh crates/ops/src/watchdog.rs
 docs/fa/ops.md | 2 +-        # все файлы, о которых говорит правка, на ветке и в main идентичны
```

Ветка отстаёт от `main` на два merge-коммита (#306, #307 — docs-only close-out `M-93`); спека `M-93` на `main` уже лежит в `docs/archive/M-93-watchdog-delivery.md`, на ветке — ещё в `milestones/`. Правка ссылается на `M-93` идентификатором, не путём, — висячей ссылки это не даёт.

Ярус S (состояние мира) снят той же сессией: `gh run list --branch main --limit 5` — все `success`; `gh run list --workflow=deploy.yml --limit 3` — `success`; VPS `git rev-parse --short HEAD` → `fd21292d` (merge PR #305 = `M-93`; #306/#307 — только документы, деплой по ним 12–15 с); `bash scripts/check_branch_health.sh` → `VERDICT: PASS`, замечаний 0.

## §1 — Что говорит правка (разбор по утверждениям)

Новый текст заменяет «Предел, названный честно: на проде потребитель НЕ ИСПОЛНЯЕТСЯ … (`TD-231`, `TD-220`)» на пять утверждений:

| # | Утверждение правки | Проверено командой | Итог |
|---|---|---|---|
| У-1 | бинарь `ops-watchdog` собирается в прод-образе | `Dockerfile:18` (`--bin ops-watchdog`), `:40` (`COPY --from=builder … /usr/local/bin/ops-watchdog`) | ✅ |
| У-2 | доставляется деплоем на хост в `/usr/local/lib/hft/ops-watchdog`; cron `*/5` зовёт его оттуда | `deploy.yml:302-305` → `deploy/bin/install-watchdog.sh` (`DST=…/usr/local/lib/hft/ops-watchdog`); `deploy/cron.d/watchdog` → `*/5 … scripts/watchdog_cron.sh`; `watchdog_cron.sh:27` `WATCHDOG_BIN=…/usr/local/lib/hft/ops-watchdog`, `:52` исполняет его. Прод: бинарь `-rwxr-xr-x … Oct 4 10:39`, sha256 на хосте == sha256 в образе `hft-recorder` (`1bf269a4…a839e`), `/etc/cron.d/hft-watchdog` — та же строка `*/5` | ✅ («оттуда» — через обёртку `/root/hft-platform/scripts/watchdog_cron.sh`; допустимое сокращение) |
| У-3 | замер 2026-10-04 — `watchdog.state.json` несёт `prev_serving_heartbeat`, тик cron'а — «норма» **(`R-222` §8)** | Прод: `grep -c prev_serving_heartbeat …state.json` → `1`; `tail -2 watchdog.log` → «норма, ни одно условие не сработало»; `last-success` `2026-10-04T17:00:01Z`. **Но** `R-222` не содержит ни §8, ни этого замера — см. Б-2 | ⚠ факт верен, **цитата ложна** |
| У-4 | до `M-93` потребитель не исполнялся (`TD-231`, закрыт) | `TECH-DEBT.md:53` («−1 `TD-231` (закрыта прод-замером…)»), `docs/archive/TECH-DEBT-closed-2026-08-16.md:2548` («`TD-231` ✅ CLOSED 2026-10-04 (reviewer, `R-222`; фикс — `M-93`, PR #305…»); `R-222` §7 п. 3 | ✅ |
| У-5 | канал наружу не включён (`П-003`) — тревога пишется **в журнал cron'а и файл тревоги**, в Telegram не уходит | `docs/PENDING-SIGNATURE.md:72-90` — `П-003` п. 1 «`TELEGRAM_BOT_TOKEN` … Что сделать вам» (не подписан); прод-лог: 654 строки «TELEGRAM_BOT_TOKEN/TELEGRAM_CHAT_ID не заданы». **Но** «файл тревоги» тревогу OPS-I-11 не получает — см. Б-1 | ⚠ половина верна, **канал назван неверно** |

## §2 — Находки

### Б-1 (блокирует). «Тревога пишется … и файл тревоги» — ложно для тревоги OPS-I-11

Что говорит код (`origin/main` == ветка в этих файлах):

```
$ sed -n 91,106p crates/ops/src/bin/ops-watchdog.rs
    let stdout_transport = StdoutTransport;
    let telegram_transport = TelegramTransport::from_env();
    …
    for alert in &outcome.delivered {
        let message = format_alert(alert, &host_label, now_ms);
        // stdout — всегда (лог cron'а — свидетельство работы watchdog'а само по себе).
        let _ = stdout_transport.send(&message);
        if let Err(e) = telegram_transport.send(&message) { … }
    }
$ grep -n -A4 'impl Transport for StdoutTransport' crates/ops/src/transport.rs
42:impl Transport for StdoutTransport {
43-    fn send(&self, message: &str) -> Result<(), TransportError> {
44-        println!("{}", message.replace('\n', " | "));
$ grep -n 'process::exit\|exit(' crates/ops/src/bin/ops-watchdog.rs     # пусто: main возвращает Ok(()) и при сработавших алертах
$ sed -n 29,60p scripts/watchdog_cron.sh
ALERT_FILE="${WATCHDOG_ALERT_FILE:-/var/lib/hft/watchdog.alert}"
alert() { …; { date -u …; echo "${msg}"; } >"${ALERT_FILE}" …; }
if [ ! -x "${WATCHDOG_BIN}" ]; then alert "ops-watchdog: бинарь не найден/не исполняем …"; exit 1; fi
"${WATCHDOG_BIN}" >>"${LOG}" 2>&1
rc=$?
if [ "${rc}" -ne 0 ]; then
  alert "ops-watchdog exit=${rc} — сам процесс мониторинга упал (не путать с найденными им CRITICAL-алертами, это НЕ ошибка). Лог: ${LOG}"
else
  rm -f "${ALERT_FILE}" 2>/dev/null || true
  date -u +%Y-%m-%dT%H:%M:%SZ >"${LAST_SUCCESS}" 2>/dev/null || true
fi
```

То есть: бинарь пишет тревоги ТОЛЬКО в stdout (→ `/var/log/hft/watchdog.log`) и, при токене, в Telegram; файла тревоги он не ведёт. Единственный «файл тревоги» в контуре — `/var/lib/hft/watchdog.alert` обёртки, и он (а) пишется только на отказ САМОЙ обёртки/процесса (бинарь не найден, `exit≠0`), о чём обёртка говорит прямо: «не путать с найденными им CRITICAL-алертами»; (б) **стирается на каждом тике с `rc=0`** — а бинарь возвращает `Ok(())` и когда CRITICAL сработал. Следствие: тревога «тишина выдачи» в файл не попадёт никогда, а если файл и был — успешный тик его снимет. На проде файла нет (`ls /var/lib/hft/watchdog.alert` → `No such file`), что согласуется с кодом.

Почему это не «уточнение формулировки». Строка OPS-I-11 — спека потребителя для оператора и для следующего milestone'а; читатель, строящий наблюдение за тревогой OPS-I-11 по «файлу тревоги», получит молчание ровно на том событии, ради которого инвариант существует. Это класс «утверждение о коде без открытия кода» — тот, против которого §9 п. (а) и заведён.

**Фикс (одна подстрока):** «тревога пишется в журнал cron'а и файл тревоги» → «тревога печатается в stdout бинаря (= журнал cron'а `/var/log/hft/watchdog.log`), файла тревоги у неё нет — `watchdog.alert` обёртки сигналит только отказ самого сторожа и снимается успешным тиком». Либо короче: «…пишется только в журнал cron'а, в Telegram не уходит».

### Б-2 (блокирует). Ссылка «(`R-222` §8)» указывает на несуществующий раздел; замер живёт не там

```
$ grep -n -E '^## |^### ' research/reviews/R-222-M-93-watchdog-delivery-pr-gate.md
22:## §0 … 33:## §1 … 50:## §2 … 56:## §3 … 91:### §3bis … 96:## §4 … 145:## §5 … 175:## §6 … 183:## §7     # §8 нет
$ grep -n -E 'prev_serving_heartbeat|норма, ни одно|10:4[0-9]Z' research/reviews/R-222-*.md
(пусто)
$ sed -n 20p research/reviews/R-222-*.md
Задача 4 (§8-гейт на проде) исполняется после merge и предъявляется в close-out.
$ sed -n 80p research/reviews/R-222-*.md
SKIP  task4: §8-гейт …
$ git grep -n 'prev_serving_heartbeat' origin/main -- PROJECT-STATE.md TECH-DEBT.md
origin/main:PROJECT-STATE.md:3058:watchdog.state.json: prev_serving_heartbeat (якорь OPS-I-11), disk_history (TD-176)
origin/main:TECH-DEBT.md:197:| **TD-220** | **ОБНОВЛЕНО reviewer'ом 2026-10-04 (`R-222`, close-out `M-93`): … прод-замер 10:45Z …
origin/main:TECH-DEBT.md:4849:  prev_serving_heartbeat {'ts_wall_ms': 1791110695295, 'attempts': 0, … }
```

`R-222` — PR-гейт ДО merge'а; он сам пишет, что §8 исполняется после merge и предъявляется в close-out. Замер (`prev_serving_heartbeat` + тик «норма» 10:45Z) записан reviewer'ом в close-out `M-93`: `PROJECT-STATE.md:3044-3058` (блок «§8 (задача 4), обе ноги») и карточки `TD-220`/`TD-176` в `TECH-DEBT.md` (коммиты `25427cd`, `8e9d771`). Ссылка «`R-222` §8» отправляет читателя в документ, где этого замера нет, — нарушение правила цитаты (`reading-map.md` §3) в документе, который сам является источником для исполняющих агентов.

**Фикс:** «(`R-222` §8)» → «(close-out `M-93`: `PROJECT-STATE.md` «§8 (задача 4)», `TECH-DEBT.md` `TD-220`)». Факт замера я повторил сам 2026-10-04T17:03Z — он держится (см. Done Block), менять нужно только адрес.

### Н-1 (не блокирует; вне диффа). §7.1 не несёт строки для OPS-I-11

`OPS-I-5` (§6) требует, чтобы каждый класс инцидента имел строку в §7.1 и правило P0/P1; `OPS-I-11` введён в §6 (`TD-237`, до этой правки), но строки «тишина выдачи → CRITICAL» в матрице §7.1 нет. Это долг предыдущей правки FA, не этой; называю, чтобы его не потеряли, в вердикт по данной ветке не входит.

### Н-2 (наблюдение, вне диффа). Шапка FA устарела

`docs/fa/ops.md:5` «OPS-I-1..9 — источник RED-оракулов» при наличии OPS-I-10/11 в §6; §O п. 1 «склоняюсь к Prometheus + Alertmanager → Telegram» при уже построенном `ops-watchdog`. Изложение, автор приземляет сам при следующем касании файла.

## §3 — (б) Полномочия

- `docs/fa/**` — зона architect'а (`scope-guard.md`); правка одной строки FA, других путей в диффе нет (`git diff --stat` выше).
- Граница C не затронута: состав записываемых данных, деньги, веса, фазы — не упоминаются и не меняются; `П-003` названа как ОТКРЫТЫЙ пункт founder'а, подпись не подменяется.
- Замок §11 (`.claude/**`, `CLAUDE.md`, `docs/04-workflow.md`) не задет; `FOUNDER-APPROVED` не требуется.
- Форма инварианта (правило `Δattempts > 0 ∧ Δrefusals_supported > 0 ∧ Δsuccesses == 0`, продюсер, потребитель, оракулы) не изменена — меняется только абзац о состоянии прода; критик по триггеру «изменение инварианта» не требуется, маршрут §9-перепроверки — верный.

## §4 — (в) Связность

- `OPS-I-8` (строка 476) по-прежнему отсылает к `OPS-I-11` как к отдельному классу — согласовано.
- `OPS-I-10` — «объявлена ⟹ эмитится»: правка усиливает согласие (потребитель теперь исполняется), не противоречит.
- Идентификаторы, названные строкой, существуют на дереве: `check_serving_silence` (`crates/ops/src/watchdog.rs:629`), `check_serving_heartbeat_missing` (`:575`), `ServingHeartbeatStale` (`:53`), `serving_heartbeat_warn_ms`/`_crit_ms` (`:218-219`); `prev_serving_heartbeat` — `crates/ops/src/state.rs:51`, `watchdog_cycle.rs:493,502`; все четыре оракула-файла присутствуют (`ls` в Done Block).
- Висячих ссылок правка не вносит: `M-93`, `TD-231`, `TD-237`, `П-003`, `R-222` — существуют; дефект Б-2 — не висячая ссылка, а неверный адрес внутри существующего документа.
- `bash scripts/verify_design_claims.sh --merge-preview origin/main` → `VERDICT: PASS (0 нарушений)`, exit=0.

## §5 — Предъявление FA

Диф не трогает `crates/**` → барьер `review-fa` даст `SKIP`; требование здесь когнитивное, выполняю: предмет — `OPS-I-11` (`docs/fa/ops.md:479`), соседний — `OPS-I-8` (`:476`); оба открыты и прочитаны целиком с файлом.

## §6 — Условие APPROVE

Одна правка той же строки `docs/fa/ops.md:479`: (1) убрать «и файл тревоги» / заменить формулировкой из Б-1; (2) заменить «(`R-222` §8)» адресом из Б-2. Больше ничего менять не нужно; повторная перепроверка — по той же §9 (свежий агент), объём — одна строка.

## Done Block

```
$ git rev-parse origin/docs/ops-i-11-prod; git rev-parse origin/main
bd7c84c471864de9cbabc2ba342e221b563de45e
f64e72fdd0427cb7d82a402ec25a5dbbc97d823f
$ git diff 74bd371ef5eb85d1adfd0d2ba88c7e02821ded26..HEAD --stat
 docs/fa/ops.md | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)

$ git show origin/main:Dockerfile | grep -n -i watchdog
18:RUN cargo build --release --bin recorder --bin journal-retention --bin gateway-serve --bin gateway-checkpoint --bin wsprobe --bin ops-watchdog
40:COPY --from=builder /build/target/release/ops-watchdog /usr/local/bin/ops-watchdog
$ git show origin/main:.github/workflows/deploy.yml | grep -n -i watchdog
303:              # РАБОТАЮЩЕГО hft-recorder и атомарно кладётся в /usr/local/lib/hft/ops-watchdog
305:              bash deploy/bin/install-watchdog.sh || { echo "=== WATCHDOG INSTALL FAILED ===" >&2; exit 1; }
$ git show origin/main:deploy/cron.d/watchdog | grep -v '^#' | grep -v '^$'
WATCHDOG_SERVING_HEARTBEAT_PATH=/var/lib/docker/volumes/hft-platform_gateway-state/_data/gateway-serve.heartbeat
*/5 * * * * root flock -n /var/lock/hft-watchdog.lock /root/hft-platform/scripts/watchdog_cron.sh
$ git show origin/main:scripts/watchdog_cron.sh | grep -n 'WATCHDOG_BIN=\|ALERT_FILE=\|^"\${WATCHDOG_BIN}"'
27:WATCHDOG_BIN="${WATCHDOG_BIN:-${HFT_WATCHDOG_ROOT:-}/usr/local/lib/hft/ops-watchdog}"
29:ALERT_FILE="${WATCHDOG_ALERT_FILE:-/var/lib/hft/watchdog.alert}"
52:"${WATCHDOG_BIN}" >>"${LOG}" 2>&1

$ ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes root@167.233.192.131 'date -u; ls -la /usr/local/lib/hft/ops-watchdog; grep -c prev_serving_heartbeat /var/lib/hft/watchdog.state.json; tail -2 /var/log/hft/watchdog.log; cat /var/lib/hft/watchdog.last-success; grep -v "^#" /etc/cron.d/hft-watchdog | grep -v "^$"; grep -c "TELEGRAM_BOT_TOKEN/TELEGRAM_CHAT_ID" /var/log/hft/watchdog.log; ls -la /var/lib/hft/watchdog.alert; docker exec hft-recorder sha256sum /usr/local/bin/ops-watchdog; sha256sum /usr/local/lib/hft/ops-watchdog'
Sun Oct  4 05:03:34 PM UTC 2026
-rwxr-xr-x 1 root root 5228496 Oct  4 10:39 /usr/local/lib/hft/ops-watchdog
1
[ops-watchdog] TELEGRAM_BOT_TOKEN/TELEGRAM_CHAT_ID не заданы — алерты идут только в stdout (лог cron'а). …
[ops-watchdog] 1791133201476 — норма, ни одно условие не сработало
2026-10-04T17:00:01Z
WATCHDOG_SERVING_HEARTBEAT_PATH=/var/lib/docker/volumes/hft-platform_gateway-state/_data/gateway-serve.heartbeat
*/5 * * * * root flock -n /var/lock/hft-watchdog.lock /root/hft-platform/scripts/watchdog_cron.sh
654
ls: cannot access '/var/lib/hft/watchdog.alert': No such file or directory
1bf269a416c52f191ae14cef0b78e5e399ab8b4b6cfffb5a010d946d227a839e  /usr/local/bin/ops-watchdog
1bf269a416c52f191ae14cef0b78e5e399ab8b4b6cfffb5a010d946d227a839e  /usr/local/lib/hft/ops-watchdog
$ ssh … 'cd /root/hft-platform && git rev-parse --short HEAD'
fd21292d
$ git log -1 --format='%h %ci %s' fd21292d
fd21292 2026-10-04 06:26:49 -0400 Merge pull request #305 from a3ka/feat/M-93-watchdog-delivery

$ grep -n 'TD-231' TECH-DEBT.md | head -1
53:> **2026-10-04 (reviewer, `R-222`, close-out `M-93`): 73 → 74.** … −1 `TD-231` (закрыта прод-замером, тело — в архив) …
$ git grep -n 'TD-231' origin/main -- docs/archive/TECH-DEBT-closed-2026-08-16.md | head -1
…:2548:- **TD-231** ✅ **CLOSED 2026-10-04** (reviewer, `R-222`; фикс — `M-93`, PR #305, merge `fd2…
$ grep -n '^## П-003' docs/PENDING-SIGNATURE.md
72:## П-003 — Два внешних действия founder'а
$ ls crates/ops/tests/red_m89_serving_silence.rs crates/gateway-serve/tests/red_m89_counters_instance.rs crates/gateway-serve/tests/red_m91_legacy_counters.rs crates/gateway-serve/tests/red_m89_heartbeat_entrypoint.rs
(все четыре существуют)

$ bash scripts/verify_design_claims.sh --merge-preview origin/main 2>&1 | grep -E '^(FAIL|VERDICT)'; echo exit=$?
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/check_branch_health.sh | tail -1
VERDICT: PASS — наблюдение состоялось (NOTE не блокируют: это наблюдатель, не барьер)
```
