<!-- GATE-META
milestone: M-92
audited_repo: a3ka/hft-platform
audited_base: 43b678fa0bf2dd71e4e4e7167c8b801ffdd6a9f5
audited_head: 9faf079a0608fefda89c2e9486dbac0255d61d7e
verdict: APPROVE
-->

# R-244 — M-92 хвост `R-240` Н-4: комментарий тома `/work` в `docker-compose.yml` — APPROVE

**Роль:** reviewer. **Дата:** 2026-10-05. **Предмет:** ветка `origin/fix/compose-workdir-comment`
(engine-dev), PR #326. Вершина взята командой (`04-workflow.md` §2):
`git fetch origin && git rev-parse origin/fix/compose-workdir-comment` → `9faf079a0608fefda89c2e9486dbac0255d61d7e`,
совпадает с SHA мандата. База = `origin/main` `43b678f` (ветка стоит ровно на вершине `main`, 1 коммит).

## §0. Что прочитано (ярус C — только грепом)

- `R-240` (источник находки) — §5 Н-4 целиком.
- `PROJECT-STATE.md` — греп `M-92|R-240`: M-92 ЗАКРЫТ (`R-242`, маркер `MS-STATE: M-92 CLOSED`
  на :3030), `R-240` Н-4 значится хвостом (:3069, :3161).
- `TECH-DEBT.md` — греп `R-240|retention-work|RETENTION_WORK_DIR`: 0 совпадений (`R-240` Н-4
  «долгом не заводится» — сходится).
- `deploy/bin/journal-retention-cron.sh` на вершине — греп `RETENTION_WORK_DIR|mkdir`.

## §1. Block-scope

Диф: `docker-compose.yml` +7/−3, только строки комментария над томом `/work` сервиса
`journal-retention`. Строка тома `"${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}:/work"` НЕ
тронута. Корневой `docker-compose.yml` — зона engine-dev (`scope-guard.md`, объявления своих
сервисов); `crates/**`, `contracts/**`, `*/tests/**`, sacred-скрипты не тронуты. Зона §9 (уставная)
не задета. **PASS.**

## §2. Block-C / Block-risk / FA

- Block-C: `crates/contracts/**` не тронут — неприменим.
- Block-risk: `risk`/`killswitch`/`oms`/`venue-*` не тронуты — risk-critic не требуется.
- FA: диапазон не трогает `crates/<name>/**` ⇒ барьер `review-fa` даёт SKIP; инвариант-ID
  называть не обязан, waiver не нужен.

## §3. Проверка содержательного утверждения комментария — ЗАМЕРОМ

Комментарий утверждает: (а) каталог создаёт cron-скрипт (`:145`); (б) при его отсутствии короткая
bind-форма compose создаёт источник сама (`root:root`, 0755).

(а) `git show origin/fix/compose-workdir-comment:deploy/bin/journal-retention-cron.sh | grep -n mkdir`
→ `145:mkdir -p "${RETENTION_WORK_DIR}" 2>/dev/null || true` — ссылка верна, вызов стоит ДО
`docker compose run` (:180). **Верно.**

(б) dev проверил `docker run -v`, а прод зовёт **`docker compose run`** — другая форма вызова
(`testing.md` «Целостность гейта» п.1). Перепроверено в ПРОД-форме, на обеих версиях:

```
# локально, Docker Compose v2.39.1, compose-файл с "${RETENTION_WORK_DIR:-<несуществующий>}:/work"
$ docker compose run --rm p ; echo exit=$?
exit=0
$ stat -c '%U:%G %a' <путь>
root:root 755

# VPS (прод), Docker Compose v5.3.1, тот же файл во временном каталоге /tmp/rprobe.*
exit=0
root:root 755
cleaned        # каталог пробы удалён; образ busybox, скачанный пробой, удалён (rmi=ok); сетей rprobe: 0
```

**Верно в обеих версиях.** Прежняя формулировка («упадёт с ошибкой монтирования ДО старта
бинаря») действительно была ложной — находка `R-240` Н-4 подтверждена и закрыта.

## §4. Поведение не меняется — доказано, а не заявлено

```
# GATEWAY_JWT_SECRET=x docker compose -f <файл ревизии> config | sha256sum
277d1f1e5fc81344  origin/main
277d1f1e5fc81344  origin/fix/compose-workdir-comment
config_exit=0 (обе)
```

Отрисованная конфигурация побайтово одинакова ⇒ правка чисто комментарная.

## §5. Находки (ни одна не блокирует)

- **Н-1 (стиль).** Ссылка `journal-retention-cron.sh:145` по номеру строки протухнет при правке
  скрипта. Автор назвал это осознанным трейд-офом; принимаю. Ссылка по содержимому
  (`mkdir -p "${RETENTION_WORK_DIR}"`) в тексте уже есть рядом — она и останется опорой.
- **Н-2 (уборка, вне предмета).** Dev сообщает о старом дереве `/tmp/hft-engine-dev-compose`
  (ветка `fix/m92-compose-workdir`) со staged-удалением `research/reviews/R-240-…md`. Аудит-трейл
  НЕ под угрозой: `R-240` (`a244955`) достижим из `origin/main` (`git merge-base --is-ancestor` →
  истина). Дерево — кандидат на `gc_worktrees.sh`; коммитить оттуда ничего нельзя (коммит удалил бы
  вердикт — барьер `check_protected_artifacts.sh` это поймал бы).
- **Н-3 (маршрут).** `R-240` передавал Н-4 architect'у «как вопрос»; исполнил engine-dev. Зона
  правки — его (`docker-compose.yml`), правка комментарная, RED не требуется (поведение не меняется,
  §4). Нарушения нет; отмечаю для трассировки.

## §6. Done Block (сырой вывод)

```
$ git fetch origin && git rev-parse origin/fix/compose-workdir-comment origin/main
9faf079a0608fefda89c2e9486dbac0255d61d7e
43b678fa0bf2dd71e4e4e7167c8b801ffdd6a9f5
$ git log --oneline origin/main..origin/fix/compose-workdir-comment
9faf079 docs(M-92): комментарий тома /work — кто создаёт каталог (R-240 Н-4) [engine-dev]
$ git show --stat 9faf079
 docker-compose.yml | 10 +++++++---
 1 file changed, 7 insertions(+), 3 deletions(-)
$ bash scripts/verify_delivery_M-08.sh >/dev/null 2>&1; echo verify_delivery_M-08=$?
verify_delivery_M-08=0
$ bash scripts/next_artifact_id.sh R
R-244
```

CI PR #326 — решение о merge принимается по коду возврата `gh pr checks 326 --watch`.

## §7. Условие и дальнейшие шаги

**APPROVE.** Дальше: `gh pr checks 326 --watch` (exit 0) → `gh pr merge 326 --merge --delete-branch`
→ `gates.md` §8 (CI/Deploy на SHA merge'а, VPS: контейнеры healthy, heartbeat свежий) →
`PROJECT-STATE.md`: хвост `R-240` Н-4 под M-92 — закрыт.
