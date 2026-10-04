<!-- GATE-META
milestone: TD-237
audited_repo: a3ka/hft-platform
audited_base: 74bd371ef5eb85d1adfd0d2ba88c7e02821ded26
audited_head: 791748e09e3ebe3fe5fbbec9337b953771670a25
verdict: APPROVE
-->

# R-225 — перепроверка `gates.md` §9, круг 2: `docs/fa/ops.md` OPS-I-11 «исполнение на проде» (ветка `docs/ops-i-11-prod`)

**Роль:** independent recheck (Fable, свежий контекст; автор правки — ведущий architect, автор круга 1 — другой recheck-агент; я не продолжаю ни того, ни другого).
**Предмет:** ветка `origin/docs/ops-i-11-prod`, вершина взята командой (`git rev-parse origin/docs/ops-i-11-prod` → `791748e…`), база — `git merge-base origin/main <вершина>` → `74bd371…`. На ветке три коммита поверх базы: `bd7c84c` (правка круга 1), `8c11ce0` (вердикт `R-224` REJECT), `791748e` (исправление по `R-224` Б-1/Б-2). Диф по `docs/fa/ops.md` — одна строка `:479` (OPS-I-11 таблицы §6).
**Объём круга:** закрытие `R-224` Б-1 и Б-2 в той же строке; наблюдения `R-224` Н-1/Н-2 — вне предмета, не судятся.

**Вердикт: APPROVE.** Обе блокирующие находки круга 1 закрыты правкой строки; каждое утверждение нового абзаца «Исполнение на проде … Предел» выдержало проверку командой на дереве ветки (файлы кода идентичны `origin/main`) и на `origin/main` для адресов замера. Полномочия в норме, висячих ссылок нет, `verify_design_claims.sh --merge-preview origin/main` → PASS.

## §0 — Вершина и дерево

```
$ git fetch origin; git rev-parse origin/docs/ops-i-11-prod
791748e09e3ebe3fe5fbbec9337b953771670a25
$ git merge-base origin/main origin/docs/ops-i-11-prod
74bd371ef5eb85d1adfd0d2ba88c7e02821ded26
$ git rev-parse origin/main
f64e72fdd0427cb7d82a402ec25a5dbbc97d823f
$ git log --oneline origin/main..origin/docs/ops-i-11-prod
791748e docs(fa/ops): OPS-I-11 — адрес прод-замера и канал тревоги по R-224 Б-1/Б-2 [architect]
8c11ce0 docs(review): R-224 — перепроверка §9 OPS-I-11 (prod) REJECT [architect-recheck]
bd7c84c docs(fa/ops): OPS-I-11 — исполнение на проде с M-93 (бинарь доставляется деплоем), предел — канал П-003 [architect]
$ git worktree add /tmp/hft-recheck-ops11b origin/docs/ops-i-11-prod && git checkout -B docs/ops-i-11-prod origin/docs/ops-i-11-prod
$ git diff --stat 74bd371e..791748e0
 docs/fa/ops.md                                  |   2 +-
 research/reviews/R-224-ops-i-11-prod-recheck.md | 198 ++++++++++++++++++++++++
$ git diff --stat origin/main HEAD -- Dockerfile deploy/ .github/workflows/deploy.yml scripts/watchdog_cron.sh crates/ops/src/
(пусто — все файлы кода, о которых говорит строка, на ветке и в main идентичны)
$ bash scripts/check_branch_health.sh | tail -1
VERDICT: PASS — наблюдение состоялось (NOTE не блокируют: это наблюдатель, не барьер)
```

## §1 — Закрытие находок круга 1

### Б-1 (`R-224`): «тревога пишется … и файл тревоги» → ЗАКРЫТО

Новый текст: «тревога печатается только в stdout бинаря, то есть в журнал cron'а `/var/log/hft/watchdog.log`, и в Telegram не уходит; файла тревоги у неё нет — `watchdog.alert` обёртки `watchdog_cron.sh` сигналит лишь отказ самого сторожа и снимается успешным тиком (`R-224` Б-1)».

Сверка с кодом (ветка == `origin/main` в этих файлах):

| Подстрока | Команда / место | Итог |
|---|---|---|
| «печатается только в stdout бинаря» | `crates/ops/src/bin/ops-watchdog.rs:101-107`: цикл по `outcome.delivered` → `stdout_transport.send` + `telegram_transport.send`; `grep -n 'File::create\|write(' ops-watchdog.rs` — записи файла тревоги нет (строки `:350,:367` — тестовые фикстуры маркера); `crates/ops/src/transport.rs:42-46` `StdoutTransport::send` = `println!` | ✅ |
| «то есть в журнал cron'а `/var/log/hft/watchdog.log`» | `scripts/watchdog_cron.sh:28` `LOG="${WATCHDOG_LOG:-/var/log/hft/watchdog.log}"`, `:52` `"${WATCHDOG_BIN}" >>"${LOG}" 2>&1` | ✅ |
| «в Telegram не уходит» (под `П-003`) | `transport.rs:127-135`: без `TELEGRAM_BOT_TOKEN`/`TELEGRAM_CHAT_ID` `send` — no-op с `Ok(())`; `docs/PENDING-SIGNATURE.md:72,76` — `П-003` п. 1 токен не выдан; прод: `grep -c TELEGRAM_BOT_TOKEN /etc/environment /root/hft-platform/.env` → `0 0` | ✅ |
| «`watchdog.alert` … сигналит лишь отказ самого сторожа» | `watchdog_cron.sh:29` `ALERT_FILE=…/var/lib/hft/watchdog.alert`; `alert()` `:34-40`; вызовы только `:47` (бинарь не исполняем) и `:56` (`rc≠0`, с текстом «не путать с найденными им CRITICAL-алертами») | ✅ |
| «снимается успешным тиком» | `watchdog_cron.sh:57-60`: `else rm -f "${ALERT_FILE}"`; бинарь при сработавших алертах возвращает `Ok(())` (`ops-watchdog.rs:33` docstring «Exit-код: 0 всегда, ЕСЛИ детекторы отработали») | ✅ |
| «обёртки `watchdog_cron.sh`» | файл существует `scripts/watchdog_cron.sh`; cron на проде зовёт именно его (`/etc/cron.d/hft-watchdog`, см. Done Block) | ✅ |

Прод (2026-10-04T17:42Z): `watchdog.alert` отсутствует, последний тик 17:40:01Z «норма», лог растёт (349 932 байт) — согласуется с кодом.

### Б-2 (`R-224`): «(`R-222` §8)» → ЗАКРЫТО

Новый адрес: «close-out `M-93`: `PROJECT-STATE.md` блок «§8 (задача 4)», `TECH-DEBT.md` `TD-220`». Ложная ссылка на `R-222 §8` из строки удалена (`grep -o 'R-222. §8' docs/fa/ops.md` → пусто, exit=1).

```
$ git grep -n '§8 (задача 4)' origin/main -- PROJECT-STATE.md
origin/main:PROJECT-STATE.md:3047:**§8 (задача 4), обе ноги:**
$ git show origin/main:PROJECT-STATE.md | sed -n 3056,3058p
тик 10:45Z: «[ops-watchdog] … — норма», watchdog.last-success 10:45:01Z, watchdog.alert снят
watchdog.state.json: prev_serving_heartbeat (якорь OPS-I-11), disk_history (TD-176)
$ git grep -n '^| \*\*TD-220\*\*' origin/main -- TECH-DEBT.md | cut -c1-300
origin/main:TECH-DEBT.md:197:| **TD-220** | **ОБНОВЛЕНО reviewer'ом 2026-10-04 (`R-222`, close-out `M-93`): потребитель ДОСТАВЛЕН И ИСПОЛНЯЕТСЯ — `ops-watchdog` едет образом, деплой ставит его в `/usr/local/lib/hft/ops-watchdog`, cron `*/5` им пользуется; прод-замер 10:45Z: `watchdog.state.json` несёт …
```

Оба адреса существуют на `origin/main` и несут именно те два факта, на которые строка ссылается (`prev_serving_heartbeat` в `watchdog.state.json` + тик «норма»). Факт замера повторён мной 2026-10-04T17:42Z (Done Block): `grep -c prev_serving_heartbeat …state.json` → `1`, тик «норма».

## §2 — (а) Остальные утверждения абзаца — командой

| # | Утверждение | Команда | Итог |
|---|---|---|---|
| У-1 | «с 2026-10-04, `M-93`»; «бинарь собирается в прод-образе» | `Dockerfile:18` `--bin ops-watchdog`, `:40` `COPY --from=builder … /usr/local/bin/ops-watchdog`; `M-93` = PR #305, merge `fd21292` 2026-10-04 (`git log -1 fd21292d`, подтверждено `R-224` §Done); на проде `git rev-parse --short HEAD` → `fd21292d` | ✅ |
| У-2 | «доставляется деплоем на хост (`/usr/local/lib/hft/ops-watchdog`), cron `*/5` зовёт его оттуда» | `deploy.yml:303-305` → `deploy/bin/install-watchdog.sh:45` `DST=…/usr/local/lib/hft/ops-watchdog`; `deploy/cron.d/watchdog`: `*/5 … scripts/watchdog_cron.sh`; `watchdog_cron.sh:27` `WATCHDOG_BIN=…/usr/local/lib/hft/ops-watchdog`; прод: `-rwxr-xr-x … Oct 4 10:39 /usr/local/lib/hft/ops-watchdog`, `/etc/cron.d/hft-watchdog` — та же строка `*/5` | ✅ («оттуда» — через обёртку; принято кругом 1, не оспариваю) |
| У-3 | «До `M-93` потребитель на проде не исполнялся (`TD-231`, закрыт)» | `origin/main:docs/archive/TECH-DEBT-closed-2026-08-16.md:2548` «**TD-231** ✅ **CLOSED 2026-10-04** (reviewer, `R-222`; фикс — `M-93`, PR #305 …)»; `origin/main:TECH-DEBT.md:53` «−1 `TD-231`» | ✅ |
| У-4 | «канал наружу не включён (`П-003`)» | `docs/PENDING-SIGNATURE.md:72` `## П-003 — Два внешних действия founder'а`, `:76` «`TELEGRAM_BOT_TOKEN` — алертинг ГОТОВ и в проде», `:131` «⏳ отложено осознанно» | ✅ |
| У-5 | ссылка «(`R-224` Б-1)» | `research/reviews/R-224-ops-i-11-prod-recheck.md:49` `### Б-1 (блокирует)` — на ветке, коммит `8c11ce0` | ✅ |

Форма таблицы не сломана: строки `:478` и `:479` несут по 3 разделителя `|`, как и прежде.

## §3 — (б) Полномочия

- Единственный файл правки — `docs/fa/ops.md` (зона architect'а, `scope-guard.md`); второй файл диффа — `research/reviews/R-224-*.md`, артефакт гейта круга 1, закоммичен ролью, его вынесшей (`branch-hygiene.md` п. 4).
- Замок §11 не задет: `.claude/**`, `CLAUDE.md`, `docs/04-workflow.md` в диффе отсутствуют; `FOUNDER-APPROVED` не требуется.
- Граница C не затронута: `П-003` названа как ОТКРЫТЫЙ пункт founder'а, подпись не подменяется; состав данных, деньги, веса, фазы — не упоминаются.
- Форма инварианта (правило на дельтах, продюсер, потребитель, оракулы) не изменена — меняется только абзац о состоянии прода и его пределе; критик по триггеру «изменение инварианта» не требуется, маршрут §9-перепроверки верный. Коммиты с ролевой меткой `[architect]`/`[architect-recheck]`, без co-author трейлеров (`git show -s --format=%B`).

## §4 — (в) Связность и ссылки

- Идентификаторы, названные абзацем, существуют на дереве: `M-93` (`docs/archive/M-93-watchdog-delivery.md` на `origin/main`; на ветке — ещё в `milestones/`, ссылка идентификатором, не путём), `TD-231`, `TD-237` (4 упоминания в `TECH-DEBT.md`), `TD-220` (`TECH-DEBT.md:197`), `П-003` (`PENDING-SIGNATURE.md:72`), `R-224` (на ветке), `watchdog_cron.sh`, `/var/log/hft/watchdog.log`, `/usr/local/lib/hft/ops-watchdog`, `watchdog.state.json`, `prev_serving_heartbeat` (`crates/ops/src/state.rs`, `PROJECT-STATE.md:3058`).
- `OPS-I-8` (`:476`) по-прежнему отсылает к `OPS-I-11` как к отдельному классу — согласовано; `OPS-I-10` («объявлена ⟹ эмитится») правкой усилен, не нарушен.
- `bash scripts/verify_design_claims.sh --merge-preview origin/main` → `VERDICT: PASS (0 нарушений)`, exit=0.
- Висячих ссылок правка не вносит; ложный адрес `R-222 §8` (Б-2) из текста удалён.

## §5 — Предъявление FA

Диф не трогает `crates/**` → барьер `review-fa` даст `SKIP`; требование здесь когнитивное. Предмет — `OPS-I-11` (`docs/fa/ops.md:479`), соседние живые инварианты того же §6 — `OPS-I-8` (`:476`, тишина ПОТОКА, не ВЫДАЧИ) и `OPS-I-10` (`:478`); строки открыты и прочитаны вместе с дифом, а не по пересказу `R-224`.

## §6 — Наблюдение вне вердикта (не требование)

- Н-1. Абзац говорит о канале доставки; дедупликация внутри бинаря (`outcome.delivered` против `outcome.fired`, `ops-watchdog.rs:101,109-117`) означает, что подавленная дедупом тревога в stdout не печатается вовсе — печатается только сводка «обнаружено N алертов (M отправлено, K подавлено)». Это свойство правила, а не канала, и предмет строки не затрагивает; фиксирую, чтобы читатель строки не ждал ПОВТОРНОЙ печати одной и той же тревоги каждый тик. Правки не требует.
- `R-224` Н-1 (строка для OPS-I-11 в §7.1) и Н-2 (устаревшая шапка FA) — по мандату вне предмета; остаются как названы в `R-224`.

## §7 — Условие APPROVE

Выполнено на вершине `791748e`: обе подстроки из `R-224` §6 заменены; других требований к строке нет. Ветка готова к вливанию в `main` своим PR (`gates.md` §9, маршрут автора).

## Done Block

```
$ git rev-parse origin/docs/ops-i-11-prod; git rev-parse origin/main; git merge-base origin/main origin/docs/ops-i-11-prod
791748e09e3ebe3fe5fbbec9337b953771670a25
f64e72fdd0427cb7d82a402ec25a5dbbc97d823f
74bd371ef5eb85d1adfd0d2ba88c7e02821ded26

$ git diff --stat origin/main HEAD -- Dockerfile deploy/ .github/workflows/deploy.yml scripts/watchdog_cron.sh crates/ops/src/
(пусто)

$ sed -n 101,107p crates/ops/src/bin/ops-watchdog.rs
    for alert in &outcome.delivered {
        let message = format_alert(alert, &host_label, now_ms);
        // stdout — всегда (лог cron'а — свидетельство работы watchdog'а само по себе).
        let _ = stdout_transport.send(&message);
        if let Err(e) = telegram_transport.send(&message) {
            eprintln!("[ops-watchdog] TelegramTransport::send failed: {e}");
        }
$ grep -n -A3 'impl Transport for StdoutTransport' crates/ops/src/transport.rs
42:impl Transport for StdoutTransport {
43-    fn send(&self, message: &str) -> Result<(), TransportError> {
44-        println!("{}", message.replace('\n', " | "));
45-        Ok(())
$ sed -n 129,135p crates/ops/src/transport.rs
        let Some((token, chat_id)) = &self.credentials else {
            eprintln!(
                "[ops::transport] TelegramTransport: транспорт не сконфигурирован (нет \
                 TELEGRAM_BOT_TOKEN/TELEGRAM_CHAT_ID в окружении) — сообщение НЕ отправлено \
                 в Telegram, no-op"
            );
            return Ok(());
$ grep -rn 'watchdog.alert' crates/ops/src/ scripts/ deploy/
scripts/watchdog_cron.sh:29:ALERT_FILE="${WATCHDOG_ALERT_FILE:-/var/lib/hft/watchdog.alert}"
deploy/README.md:126:sudo rm -f /var/lib/hft/watchdog.alert
$ sed -n 27,29p scripts/watchdog_cron.sh; sed -n 52,60p scripts/watchdog_cron.sh
WATCHDOG_BIN="${WATCHDOG_BIN:-${HFT_WATCHDOG_ROOT:-}/usr/local/lib/hft/ops-watchdog}"
LOG="${WATCHDOG_LOG:-/var/log/hft/watchdog.log}"
ALERT_FILE="${WATCHDOG_ALERT_FILE:-/var/lib/hft/watchdog.alert}"
"${WATCHDOG_BIN}" >>"${LOG}" 2>&1
rc=$?

if [ "${rc}" -ne 0 ]; then
  alert "ops-watchdog exit=${rc} — сам процесс мониторинга упал (не путать с найденными им CRITICAL-алертами, это НЕ ошибка). Лог: ${LOG}"
else
  rm -f "${ALERT_FILE}" 2>/dev/null || true
  date -u +%Y-%m-%dT%H:%M:%SZ >"${LAST_SUCCESS}" 2>/dev/null || true
fi

$ grep -n -i watchdog Dockerfile | grep -v '^3[67]:'
18:RUN cargo build --release --bin recorder --bin journal-retention --bin gateway-serve --bin gateway-checkpoint --bin wsprobe --bin ops-watchdog
40:COPY --from=builder /build/target/release/ops-watchdog /usr/local/bin/ops-watchdog
$ grep -n 'install-watchdog' .github/workflows/deploy.yml | head -1; grep -n '^DST=' deploy/bin/install-watchdog.sh
305:              bash deploy/bin/install-watchdog.sh || { echo "=== WATCHDOG INSTALL FAILED ===" >&2; exit 1; }
45:DST="${HFT_WATCHDOG_DST:-${HFT_WATCHDOG_ROOT:-}/usr/local/lib/hft/ops-watchdog}"
$ grep -v '^#' deploy/cron.d/watchdog | grep -v '^$' | tail -1
*/5 * * * * root flock -n /var/lock/hft-watchdog.lock /root/hft-platform/scripts/watchdog_cron.sh

$ git grep -n '§8 (задача 4)' origin/main -- PROJECT-STATE.md
origin/main:PROJECT-STATE.md:3047:**§8 (задача 4), обе ноги:**
$ git grep -n 'prev_serving_heartbeat' origin/main -- PROJECT-STATE.md
origin/main:PROJECT-STATE.md:3058:watchdog.state.json: prev_serving_heartbeat (якорь OPS-I-11), disk_history (TD-176)
$ git grep -n '^| \*\*TD-220\*\*' origin/main -- TECH-DEBT.md | cut -c1-120
origin/main:TECH-DEBT.md:197:| **TD-220** | **ОБНОВЛЕНО reviewer'ом 2026-10-04 (`R-222`, close-out `M-93`): потребитель ДОСТАВЛЕН
$ git grep -n 'TD-231\*\* ✅' origin/main -- docs/archive/ | cut -c1-140
origin/main:docs/archive/TECH-DEBT-closed-2026-08-16.md:2548:- **TD-231** ✅ **CLOSED 2026-10-04** (reviewer, `R-222`; фикс — `M-93`, PR #305, merge `fd21292`)
$ grep -n '^## П-003' docs/PENDING-SIGNATURE.md
72:## П-003 — Два внешних действия founder'а
$ git ls-tree --name-only origin/main docs/archive/ | grep 'M-93'
docs/archive/M-93-watchdog-delivery.md
$ grep -n '^### Б-1' research/reviews/R-224-ops-i-11-prod-recheck.md
49:### Б-1 (блокирует). «Тревога пишется … и файл тревоги» — ложно для тревоги OPS-I-11
$ grep -c 'файл тревоги' docs/fa/ops.md; grep -n -o 'R-222. §8' docs/fa/ops.md; echo exit=$?
0
exit=1
$ awk 'NR==478||NR==479{n=gsub(/\|/,"|"); print NR" pipes="n}' docs/fa/ops.md
478 pipes=3
479 pipes=3

$ ssh -i /home/nous/.ssh/hft_deploy -o IdentitiesOnly=yes root@167.233.192.131 'date -u; grep -v "^#" /etc/cron.d/hft-watchdog | grep -v "^$" | tail -1; ls -la /usr/local/lib/hft/ops-watchdog /var/log/hft/watchdog.log; tail -1 /var/log/hft/watchdog.log; cat /var/lib/hft/watchdog.last-success; ls /var/lib/hft/watchdog.alert 2>&1; grep -c prev_serving_heartbeat /var/lib/hft/watchdog.state.json; grep -c "TELEGRAM_BOT_TOKEN" /etc/environment /root/hft-platform/.env 2>&1 | tr "\n" " "; echo; cd /root/hft-platform && git rev-parse --short HEAD'
Sun Oct  4 05:42:11 PM UTC 2026
*/5 * * * * root flock -n /var/lock/hft-watchdog.lock /root/hft-platform/scripts/watchdog_cron.sh
-rwxr-xr-x 1 root root 5228496 Oct  4 10:39 /usr/local/lib/hft/ops-watchdog
-rw-r--r-- 1 root root  349932 Oct  4 17:40 /var/log/hft/watchdog.log
[ops-watchdog] 1791135601827 — норма, ни одно условие не сработало
2026-10-04T17:40:01Z
ls: cannot access '/var/lib/hft/watchdog.alert': No such file or directory
1
/etc/environment:0 /root/hft-platform/.env:0
fd21292d
ssh exit=0

$ bash scripts/verify_design_claims.sh --merge-preview origin/main 2>&1 | grep -E '^(FAIL|VERDICT)'; echo "exit=${PIPESTATUS[0]}"
VERDICT: PASS (0 нарушений)
exit=0

$ bash scripts/check_branch_health.sh | tail -1
VERDICT: PASS — наблюдение состоялось (NOTE не блокируют: это наблюдатель, не барьер)

$ bash scripts/next_artifact_id.sh R
R-225
```
