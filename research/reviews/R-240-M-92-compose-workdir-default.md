<!-- GATE-META
milestone: M-92
audited_repo: a3ka/hft-platform
audited_base: 3753efb5b30305466ca71280d28832cea060a712
audited_head: 2dcea69dab785c0a4f4e70bc8293114ab63c7d7b
verdict: APPROVE
-->

# R-240 — M-92: умолчание `RETENTION_WORK_DIR` в compose (деплой `main` упал)

**Дата (UTC):** 2026-10-04 · **Роль:** reviewer (PR-гейт) · **Предмет:** ветка
`origin/fix/m92-compose-workdir`.

## §0. Вершина взята командой (`04-workflow.md` §2)

```
$ git fetch origin && git rev-parse origin/fix/m92-compose-workdir origin/main
2dcea69dab785c0a4f4e70bc8293114ab63c7d7b
3753efb5b30305466ca71280d28832cea060a712
$ git merge-base origin/main origin/fix/m92-compose-workdir
3753efb5b30305466ca71280d28832cea060a712
```

SHA из мандата — `2dcea69`, совпадает с вершиной. Ветка стоит прямо на вершине `origin/main`:
дерево слияния равно дереву ветки, отдельный прогон на дереве слияния не нужен (`gates.md` §8).

## §1. Вердикт: APPROVE

Два коммита, RED раньше GREEN:

| SHA | роль | файлы |
|---|---|---|
| `a572c2e` | architect | `scripts/verify_delivery_M-08.sh` +35 (шаг D10) |
| `2dcea69` | engine-dev | `docker-compose.yml` 1/1 |

Правка: `"${RETENTION_WORK_DIR}:/work"` → `"${RETENTION_WORK_DIR:-/var/lib/hft/retention-work}:/work"`
(сервис `journal-retention`). Умолчание совпадает с тем, что уже стоит в остальных местах:
`deploy/cron.d/journal-retention:64`, `deploy/bin/journal-retention-cron.sh:80`, комментарий
`docker-compose.yml:95`.

## §2. Блоки

**Block-scope — PASS.** `docker-compose.yml` (сервис `journal-retention`) есть в
`milestones/M-92-manifest-verified-prune.md` §7 в списке engine-dev и в зоне engine-dev по
`scope-guard.md` (корневой compose — ручки своих сервисов). `scripts/verify_delivery_M-08.sh` —
зона architect'а (`scripts/verify_*.sh`); в §7 M-92 его нет, но он есть в §8 задаче 6 (architect).
Это то же расхождение §7↔§8, что было раньше, а не новый выход за зону. Rust, `contracts`,
`risk`/`killswitch`/`oms`/`venue-*` не тронуты.

**Block-C — N/A.** `crates/contracts/**` не тронут.

**Block-risk — N/A.** Пути из RISK-BLOCK не тронуты, `risk-critic` не нужен.

**RED-first — PASS.** Тест `a572c2e` лежит раньше фикса `2dcea69`. Dev тест не трогал:
`git show --numstat 2dcea69` = `1 1 docker-compose.yml`.

**Мутации (сам прогнал на ветке и откатил):**

```
-      - "${RETENTION_WORK_DIR}:/work"                          (как в main)
FAIL  D10 compose НЕ разбирается в окружении деплоя (только GATEWAY_JWT_SECRET/GATEWAY_BANDS) — деплой уп…
DELIVERY: FAIL (1)
exit=1
-      - "${RETENTION_WORK_DIR:-/var/lib/hft/retention-wrk}:/work"   (неверное умолчание)
FAIL  D10 источник /work у journal-retention = '/var/lib/hft/retention-wrk', ожидался /var/lib/hft/retention-work …
DELIVERY: FAIL (1)
exit=1
$ git diff --quiet && echo tree-clean
tree-clean
```

D10 проверяет КЛАСС ошибки, а не одну строку: окружение очищено до двух имён, `.env`
репозитория не читается, разбирается весь файл (`--profile "*"`). Любая другая переменная
без умолчания в любом сервисе тоже его покраснит.

**Прод-форма (`testing.md` «4 свойства» п.1) — снято замером.** Деплой
`37242446986`, лог: `invalid spec: :/work: empty section between colons` → `DEPLOY FAILED —
rollback to 087ba995`. На VPS `.env` содержит ровно `GATEWAY_JWT_SECRET`, `GATEWAY_BANDS`, то есть
D10 воспроизводит именно эту форму. Compose-файл ветки разобран на самом VPS с его настоящим `.env`:

```
$ ssh … 'cd /root/hft-platform && cat > /tmp/r238-compose.yml && docker compose -f /tmp/r238-compose.yml
        --project-directory /root/hft-platform --profile "*" config --quiet; echo branch_rc=$?' < docker-compose.yml
branch_rc=0
```

## §3. Done Block (сырой, агрегированный)

```
$ HFT_DELIVERY_DEEP=1 bash scripts/verify_delivery_M-08.sh; echo exit=$?      (ветка, 2dcea69)
exit=0
PASS  D1 … PASS  D2 … PASS  D1-deep … PASS  D2-deep … PASS  D9-deep … PASS  D3 … PASS  D4 …
PASS  D5a … PASS  D5 … PASS  D7 … PASS  D6 … PASS  D6b … PASS  D8 ×2 … PASS  D9 …
PASS  D10 compose разбирается в окружении деплоя; /work ← /var/lib/hft/retention-…
DELIVERY: PASS
```

Rust не тронут, поэтому fmt/clippy/test я не гонял. Их прогонит CI на PR через агрегат
`All checks passed`, и merge идёт только по его коду возврата.

## §4. FA

Диф не трогает `crates/**`, поэтому `check_review_fa.sh` здесь даёт SKIP (`:57`). По существу
предмет — путь ретеншена `JR-I-13` (`docs/fa/journal.md`: локальный сегмент удаляется только
при равенстве sha256 сумме офсайт-копии). Рабочий каталог `/work` — носитель плана и манифеста
этого пути; правка его не меняет, только даёт значение по умолчанию, когда переменная не задана.

Ярус C, искал грепом: `PROJECT-STATE.md` — `M-92` (раздела милестоуна нет, `MS-STATE: M-92` нет);
`TECH-DEBT.md` — `RETENTION_WORK_DIR`, `37242446986` — пусто.

## §5. Замечания (не блокируют)

- **Н-1 (§9, `COGNITIVE-ONLY`).** D10 — правка харнесса (`scripts/verify_*.sh`) в зоне
  перепроверки `gates.md` §9 независимым Fable. Такой перепроверки в цепочке нет, а я не Fable.
  Пункты (а)–(в) закрыл сам: утверждения о коде — командами выше, полномочия — §2, висячие
  ссылки — `R-237` на `main` есть. Пропускаю, потому что это инцидентный фикс: `main` не
  деплоится, а прод стоит на откате. Если architect считает перепроверку нужной, её можно
  сделать после merge.
- **Н-2.** В теле `2dcea69` стоит `FA-WAIVER: docker-compose.yml — …`. Барьер читает waiver
  только из ФАЙЛА вердикта и только вида `crates/<name>` (`TD-165`). Строка инертна: вреда нет,
  пользы тоже нет.
- **Н-3.** Handoff dev'а предлагал `gh pr merge --squash`. Норма — `--merge --delete-branch`
  (`gates.md` §8, профиль reviewer'а). Мержу по норме.
- **Н-4 (вне предмета, НЕ правлю).** Комментарий `docker-compose.yml:96-97` утверждает, что без
  каталога `docker compose run` «упадёт с ошибкой монтирования». Для короткой формы bind-тома
  Docker обычно сам создаёт отсутствующий каталог хоста. Если так, заявленный fail-closed не
  наступает. На проде это безвредно: cron-обёртка делает `mkdir -p` (`journal-retention-cron.sh:145`).
  Передаю architect'у как вопрос, долгом не заводится.

## §6. Условие и дальнейшие шаги

APPROVE. Дальше: PR → `gh pr checks --watch` по коду возврата → `gh pr merge --merge
--delete-branch` → `gates.md` §8 (Deploy success на SHA merge'а, VPS на новом SHA, контейнеры
healthy, heartbeat свежий).
