<!-- GATE-META
milestone: PR-347
audited_repo: a3ka/hft-platform
audited_base: f7ab8fe2e6956b5d0bbbf06b8383d8dcb36fbf75
audited_head: dae0078e0207387813784e437787513fe1f7d89a
verdict: APPROVE
-->

# R-261 — PR-гейт: RUSTSEC-2026-0338..0342, подъём `quinn-proto` до 0.11.19

**Роль:** reviewer · **Дата:** 2026-10-12 · **PR:** #347 ·
**Ветка:** `fix/rustsec-quinn-proto` · **Автор правки:** engine-dev · **Вердикт: APPROVED**

Прецедент формы — `R-181` (PR #185, `585b8eba`, RUSTSEC-2026-0285 / `rustls`).

---

## §0 — Предмет и вершина

```
$ git fetch origin && git rev-parse origin/fix/rustsec-quinn-proto
dae0078e0207387813784e437787513fe1f7d89a        # справка мандата: dae0078e — совпала

$ git merge-base origin/main origin/fix/rustsec-quinn-proto
f7ab8fe2e6956b5d0bbbf06b8383d8dcb36fbf75        # = origin/main на момент гейта, ветка не отстаёт
```

Повторное взятие вершины перед записью вердикта — та же `dae0078e`; ветка не уходила.

Джоб `security` (`cargo audit`) входит в агрегат `All checks passed`, а агрегат — обязательный
чек защиты `main`. Публикация пяти advisory на `quinn-proto` 0.11.16 закрыла merge любого PR.

## §1 — Block-scope

```
$ git log origin/main..origin/fix/rustsec-quinn-proto --format='%h %s'
dae0078e fix(deps): RUSTSEC-2026-0339..0342 — quinn-proto >= 0.11.18, агрегат CI разблокирован [engine-dev]

$ git diff --stat origin/main...origin/fix/rustsec-quinn-proto
 Cargo.lock | 4 ++--
 1 file changed, 2 insertions(+), 2 deletions(-)

$ git diff origin/main -- Cargo.lock | grep -E '^[+-][^+-]'
-version = "0.11.16"
+version = "0.11.19"
-checksum = "2f4bfc015262b9df63c8845072ce59068853ff5872180c2ce2f13038b970e560"
+checksum = "0e750cca55fe4f0439a15d0bb529da9651e79993e8e72c61a899a36d462befbe"
```

Один коммит, один файл, одна запись пакета (`quinn-proto`). Ни один `Cargo.toml` (корневой
или крейтовый) не тронут; `crates/**/src/`, `*/tests/`, `contracts/**`, `scripts/**` — не
тронуты. Подъём semver-совместимый (0.11.x), транзитивный.

**Block-scope: ЧИСТО.**

## §2 — Block-C (контракты)

`crates/contracts/**` в диапазоне отсутствует ⇒ contract-RFC не требуется. **Block-C: неприменим.**

## §3 — Block-risk

`crates/risk/**`, `crates/killswitch/**`, `crates/oms/**`, `crates/venue-*/**` в диапазоне
отсутствуют ⇒ RISK-BLOCK `gates.md` §5 не срабатывает, вердикт `risk-critic` не требуется.

Дополнительно установлено, что пакет вообще **не входит в сборку** ни одного крейта
рабочего пространства — поведенческого эффекта на прод правка иметь не может:

```
$ cargo tree -q -i quinn-proto --target all --workspace --all-features -e all
{пусто}

# кто ссылается на quinn в Cargo.lock:
$ awk '/^\[\[package\]\]/{n=""} /^name = /{n=$3} /^ "quinn"/{print n}' Cargo.lock
"reqwest"

# после полного `cargo test --workspace` в target/:
$ ls target/debug/deps | grep -c quinn
0
```

`quinn` — опциональная (HTTP/3) зависимость `reqwest`; фича у нас не включена, но lock-файл
резолвит её всё равно, и `cargo audit` сканирует lock, а не граф сборки.

**Block-risk: неприменим; эффект на рантайм — нулевой по замеру.**

## §4 — Block-DoneBlock (сырой stdout, независимый прогон reviewer'а)

Прогон на чистом отдельном дереве `/tmp/hft-reviewer-pr347` (@ `dae0078e`), не в дереве автора:

```
$ git log -1 --oneline
dae0078e fix(deps): RUSTSEC-2026-0339..0342 — quinn-proto >= 0.11.18, агрегат CI разблокирован [engine-dev]

$ git status --porcelain
{пусто}

$ cargo audit
Crate:     atomic-polyfill   Version: 1.0.3   ID: RUSTSEC-2023-0089
Crate:     chacha20          Version: 0.10.1
Crate:     spin              Version: 0.9.8
warning: 3 allowed warnings found
exit=0

$ cargo test --workspace --no-fail-fast 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed="p" failed="f" (блоков: "NR")"}'
passed=1245 failed=0 (блоков: 281)
test_exit=0
```

Три оставшихся предупреждения — те же allow-листные, что в `R-181` (`unmaintained`/`yanked`),
уязвимостей — ноль. fmt/clippy — по CI-джобу `fmt + clippy + test` (pass, 17m22s) на этой же
вершине, см. §6.

## §5 — Анти-плацебо: тот же гейт КРАСНЫЙ против пред-фиксного lock'а

```
$ git show origin/main:Cargo.lock > /tmp/rev347-main/Cargo.lock
$ cargo audit -f /tmp/rev347-main/Cargo.lock
Crate: quinn-proto  Version: 0.11.16  ID: RUSTSEC-2026-0342  Solution: Upgrade to >=0.11.17
Crate: quinn-proto  Version: 0.11.16  ID: RUSTSEC-2026-0339  Solution: Upgrade to >=0.11.17
Crate: quinn-proto  Version: 0.11.16  ID: RUSTSEC-2026-0341  Solution: Upgrade to >=0.11.17
Crate: quinn-proto  Version: 0.11.16  ID: RUSTSEC-2026-0338  Solution: Upgrade to >=0.11.18
Crate: quinn-proto  Version: 0.11.16  ID: RUSTSEC-2026-0340  Solution: Upgrade to >=0.11.18
error: 5 vulnerabilities found!
exit=1
```

Гейт краснеет ровно на пред-фиксном состоянии и зеленеет на фиксе. Максимальное требование
advisory — `>=0.11.18`; поставлено **0.11.19**. В отличие от `R-181` (ровно минимальная
версия), здесь — последняя патч-версия той же минорной ветки, что даёт `cargo update -p
quinn-proto`; семвер-совместимо, прочих пакетов не задело (§1).

## §6 — CI на вершине

```
$ gh pr checks 347
All checks passed      pass   6s
cargo audit            pass   3m12s
fmt + clippy + test    pass   17m22s
... (прочие 18 джобов) pass
```

Решение о merge принимается по коду возврата `gh pr checks 347 --watch` после коммита этого
вердикта на ветку (коммит перезапускает CI).

## §7 — Находки

**N-1 (косметика, не блокирует).** Subject коммита и заголовок PR называют
`RUSTSEC-2026-0339..0342`; фактический набор — **0338..0342** (пять advisory, см. §5). Тело PR
это частично оговаривает («и ещё одна»). На корректность правки не влияет; история не
переписывается — фиксирую здесь, чтобы номер 0338 был найден грепом.

**N-2 (долг, не к этому PR) — третий живой случай `TD-087`.** `main` снова покраснел без
коммита в репозиторий: публикация advisory наверху закрыла merge. Класс: R-025 (сеть, 503),
R-181 (`rustls`), теперь этот. Новое в этом случае и усиливающее карточку: заблокировал пакет,
которого **нет в графе сборки** (§3) — `cargo audit` сканирует `Cargo.lock`, включая
нерезолвленные фичами опциональные зависимости, то есть гейт останавливает работу на
уязвимости, недостижимой из наших бинарей. Описание — моё, решение (кэш advisory-db /
неблокирующий audit / аудит по графу сборки, а не по lock'у / иное) — architect'а
(`gates.md` §4, граница reviewer↔architect). По `gates.md` («после третьего повторения класса
прозу писать запрещено») карточка `TD-087` дополняется этим случаем в close-out и
передаётся architect'у как кандидат на механизацию.

## §8 — FA

Диапазон не трогает `crates/<name>/**` (единственный файл — корневой `Cargo.lock`), барьер
`scripts/check_review_fa.sh` даёт `SKIP` (`:57`); называть инвариант модуля не из чего,
`FA-WAIVER` не требуется. Предел назван явно.

---

## ВЕРДИКТ: **APPROVED**

Правка минимальна (один пакет в lock-файле), предъявлена красным-и-зелёным прогоном одного
гейта, не выходит за зону, не трогает ни контракты, ни safety-путь, в граф сборки не входит.
Блокеров нет.

**Условие APPROVED:** merge через PR #347 по зелёному `All checks passed` (решение — по коду
возврата `gh pr checks 347 --watch`), затем post-merge деплой-гейт `gates.md` §8: CI + Deploy
до терминального статуса, eyes-on VPS (контейнеры `healthy`, свежий heartbeat, рост журнала).
