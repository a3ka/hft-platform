#!/usr/bin/env bash
# Acceptance-гейт M-52 — journal hardening: JR-I-10 / JR-I-11 / JR-I-12.
#
# Закрывает три открытых долга последних щелей надёжности журнала:
#   TD-052 (+TD-054) — пол защиты ограничен по ПАМЯТИ, но не по ВРЕМЕНИ;
#   TD-030            — нет машинного guard'а монотонности сшивки сегментов;
#   TD-067            — `replay_digest` не доставляется в прод.
#
# Агрегатор с FAIL-счётчиком (НЕ `set -e`: первый FAIL не должен скрывать остальные).
set -uo pipefail

cd "$(dirname "$0")/.." || exit 1
FAILS=0
SEG=crates/journal/src/segments.rs
JLIB=crates/journal/src/lib.rs
BIN=crates/journal/src/bin/journal-retention.rs
REC=crates/recorder/src

ok()  { echo "PASS  $1"; }
bad() { echo "FAIL  $1"; FAILS=$((FAILS + 1)); }

# Тело функции: от строки-заголовка до первой закрывающей скобки нулевого отступа.
# Сопоставление ЛИТЕРАЛЬНОЕ (index==1): сигнатуры содержат `(` и `<`, а POSIX-awk трактует
# их как синтаксис регулярного выражения — на этом уже был ложный PASS в M-40.
fn_body() {
  awk -v pat="$2" '
    index($0, pat) == 1 {f = 1}
    f {print}
    f && /^\}/ {exit}
  ' "$1" | sed 's://.*::'
}

# Счётчик совпадений БЕЗ `|| echo 0`: при нуле grep печатает "0" И выходит с кодом 1,
# поэтому `||` добавил бы ВТОРОЙ "0" и сравнение `-eq 0` дало бы ЛОЖНЫЙ PASS (поймано на
# RED-состоянии M-51).
count() { local n; n=$(grep -c "$1" "$2" 2>/dev/null | head -1); echo "${n:-0}"; }

echo "=== M-52 acceptance — journal hardening (JR-I-10/11/12) ==="
echo

# ══════════════════════════════════════════════════════════════════════════════════════
# Задача 1 — JR-I-10: именованный бюджет РАБОТЫ скана пола
# ══════════════════════════════════════════════════════════════════════════════════════
BUDGET_NAME='READABLE_FLOOR_WORK_BUDGET_BYTES'
if [ "$(count "const $BUDGET_NAME" "$SEG")" -eq 0 ]; then
  bad "T1 константа бюджета $BUDGET_NAME не найдена в $SEG — работа скана пола
      по-прежнему ничем не ограничена (TD-052: прод 158 сегментов / 26G сжатых, путь
      входится под инцидентом; TD-054: замер 16 MiB мусора = 384.94 s)"
else
  ok "T1 бюджет $BUDGET_NAME объявлен в $SEG"
  # Бюджет обязан быть КОНСТАНТОЙ, а не функцией размера каталога: «бюджет = размер
  # каталога» закрывал бы оракулы и не закрывал бы долг.
  BLINE=$(grep -m1 "const $BUDGET_NAME" "$SEG")
  case "$BLINE" in
    *"len()"*|*"metadata"*|*"size"*)
      bad "T1 бюджет выведен из размера каталога/файла ($BLINE) — это не граница" ;;
    *) ok "T1 бюджет — константа, не функция размера каталога" ;;
  esac
  # Нижняя граница: одного ЗДОРОВОГО сегмента прод-размера (DEFAULT_MAX_SEGMENT_BYTES,
  # 1 GiB) обязано хватать, иначе escape-hatch M-49 умирает на первом же restore.
  if printf '%s' "$BLINE" | grep -qE 'DEFAULT_MAX_SEGMENT_BYTES|[0-9]+ \* 1024 \* 1024 \* 1024'; then
    ok "T1 бюджет выражен в единицах не меньше сегмента (GiB / DEFAULT_MAX_SEGMENT_BYTES)"
  else
    bad "T1 бюджет меньше одного прод-сегмента (1 GiB) или выражен неявно: $BLINE —
        честная декларация на здоровом restore перестанет проходить (op_1/wb_3)"
  fi
fi

# Бюджет обязан списываться И на side-верификации крупного кандидата — иначе TD-054
# (перечитывание десятков MiB на каждую 64-ю позицию) остаётся вне границы.
VLF="$(fn_body "$SEG" 'fn verify_large_frame')"
if [ -z "$VLF" ]; then
  bad "T1 канарейка не смогла извлечь тело verify_large_frame (переименована?) — гейт НЕ
      проверен; правь канарейку, а не игнорируй"
elif printf '%s' "$VLF" | grep -qi 'budget'; then
  ok "T1 side-верификация крупного кандидата списывает работу в бюджет (TD-054 внутри границы)"
else
  bad "T1 verify_large_frame не видит бюджета — механизм TD-054 (сверхлинейное
      перечитывание, ×2114 на 16 MiB) остаётся НЕограниченным"
fi

# ══════════════════════════════════════════════════════════════════════════════════════
# Задача 2 — JR-I-10: исчерпание бюджета даёт Unknown, а не Known
# ══════════════════════════════════════════════════════════════════════════════════════
RF="$(fn_body "$SEG" 'fn readable_floor')"
if [ -z "$RF" ]; then
  bad "T2 канарейка не смогла извлечь тело readable_floor — гейт НЕ проверен"
else
  if printf '%s' "$RF" | grep -q 'Unknown'; then
    ok "T2 readable_floor умеет отвечать Unknown при исчерпании бюджета"
  else
    bad "T2 в readable_floor нет пути в Unknown — исчерпание бюджета некуда деградировать,
        а `Known` по частично просмотренному каталогу = заниженный пол = seq-reuse"
  fi
  if printf '%s' "$RF" | grep -qi 'budget'; then
    ok "T2 readable_floor заводит бюджет обхода КАТАЛОГА (а не только одного сегмента)"
  else
    bad "T2 readable_floor не заводит бюджет — ограничен максимум один сегмент, а стоимость
        живёт в обходе всего каталога (прод: 158 сегментов)"
  fi
fi

# ══════════════════════════════════════════════════════════════════════════════════════
# Задача 3 — JR-I-11: guard монотонности покрывает ВСЕ три пути
# ══════════════════════════════════════════════════════════════════════════════════════
if [ "$(count 'fn check_first_seq_monotonic' "$SEG")" -eq 0 ]; then
  bad "T3 guard монотонности не найден в $SEG — re-stitch архива в живой каталог остаётся
      ТИХИМ беспорядком seq (TD-030), защита = операторская дисциплина, не барьер"
else
  ok "T3 guard монотонности объявлен в $SEG"
fi
# `segments()` — общий вход stream/stream_from.
SG="$(fn_body "$SEG" 'pub fn segments')"
if printf '%s' "$SG" | grep -q 'check_first_seq_monotonic'; then
  ok "T3 guard включён в segments() ⇒ покрывает stream/stream_from"
else
  bad "T3 segments() не зовёт guard — прод-путь чтения (реплей/gateway/research) продолжит
      сшивать немонотонный каталог молча"
fi
RA="$(fn_body "$JLIB" 'pub fn read_all')"
if printf '%s' "$RA" | grep -q 'monotonic'; then
  ok "T3 guard включён в read_all"
else
  bad "T3 read_all не зовёт guard (офлайн-диагностика тоже обязана отказывать: именно она
      строит отчёты и probe'ы)"
fi
if printf '%s' "$RF" | grep -q 'monotonic'; then
  ok "T3 guard включён в readable_floor (условие закрытия TD-030 из R-002/R-003)"
else
  bad "T3 readable_floor не зовёт guard — останется путь, где немонотонность даёт
      ЗАНИЖЕННЫЙ пол защиты операторской декларации (fail-open ⇒ seq-reuse).
      Это ЯВНОЕ условие закрытия TD-030, принятое reviewer'ом в R-002 и R-003"
fi

# ══════════════════════════════════════════════════════════════════════════════════════
# Задача 4 — JR-I-11: legacy-сентинел исключается ПО schema_version, не по значению 0
# ══════════════════════════════════════════════════════════════════════════════════════
GB="$(fn_body "$SEG" 'pub(crate) fn check_first_seq_monotonic')"
[ -z "$GB" ] && GB="$(fn_body "$SEG" 'fn check_first_seq_monotonic')"
if [ -z "$GB" ]; then
  bad "T4 канарейка не смогла извлечь тело guard'а — гейт НЕ проверен"
else
  if printf '%s' "$GB" | grep -q 'SCHEMA_VERSION_PRE_HEADER'; then
    ok "T4 legacy исключается по schema_version (класс TD-011 обойдён)"
  else
    bad "T4 guard не различает legacy по SCHEMA_VERSION_PRE_HEADER. Сентинел first_seq=0 —
        это «неизвестно», а не факт; наивный guard уронит ЧТЕНИЕ боевого каталога
        (journal.legacy.json лежит на проде). Отличать по ЗНАЧЕНИЮ 0 нельзя: у первого
        v2-сегмента здорового журнала first_seq тоже 0"
  fi
  if printf '%s' "$GB" | grep -qE 'carries_events|is_empty|has_events|carries|empty'; then
    ok "T4 равенство first_seq имеет carve-out на ПУСТОЙ сегмент (JR-I-8 случай 3;
        без него падает crates/gateway — проверено прогоном прототипа)"
  else
    bad "T4 guard не различает ПУСТОЙ сегмент. `first_seq` сегмента без событий — обещание,
        а не факт, и законно РАВЕН следующему (JR-I-8, легитимный случай 3). Голое «строго
        возрастает» роняет 6 тестов crates/gateway (max_segment_bytes мал ⇒ заголовок
        переполняет сегмент ⇒ сегменты с нулём событий)"
  fi
  if printf '%s' "$GB" | grep -qE 'first_seq *== *0|first_seq *!= *0'; then
    bad "T4 guard сравнивает first_seq с нулём — ровно тот наивный признак legacy, который
        выключил бы защиту на самом частом каталоге"
  else
    ok "T4 guard не опознаёт legacy по значению first_seq == 0"
  fi
fi

# ══════════════════════════════════════════════════════════════════════════════════════
# Задача 5 — JR-I-12: режим replay-digest в УЖЕ ДОСТАВЛЯЕМОМ бинаре
# ══════════════════════════════════════════════════════════════════════════════════════
if grep -q 'replay-digest' "$BIN"; then
  ok "T5 режим replay-digest есть в journal-retention (бинарь уже в образе)"
else
  bad "T5 в $BIN нет режима replay-digest — на VPS нет Rust toolchain, и позвать
      journal::replay_digest в проде НЕЧЕМ (TD-067: детерминизм доказан в лаборатории и
      не наблюдается в поле)"
fi
if grep -q 'replay-digest' Dockerfile || grep -q 'journal-retention' Dockerfile; then
  ok "T5 бинарь, несущий режим, собирается и копируется в образ (Dockerfile)"
else
  bad "T5 бинарь с режимом не попадает в образ — режим недоставлен"
fi
if grep -q 'journal.replay-digest.json' "$BIN"; then
  ok "T5 машинная запись journal.replay-digest.json пишется рядом с журналом"
else
  bad "T5 нет машинной записи journal.replay-digest.json — cron/оператор смогут сравнивать
      прогоны только парсингом человеческого лога"
fi
HELP="$(fn_body "$BIN" 'fn print_help')"
if printf '%s' "$HELP" | grep -q 'replay-digest'; then
  ok "T5 --help называет режим (TD-024: --help не имеет права врать про контракт)"
else
  bad "T5 --help не называет replay-digest — оператор под инцидентом читает именно его"
fi

# ══════════════════════════════════════════════════════════════════════════════════════
# Задача 6 — JR-I-12: дайджест НЕ в горячем пути сбора
# ══════════════════════════════════════════════════════════════════════════════════════
if [ -d "$REC" ]; then
  if grep -rq 'replay_digest' "$REC"; then
    bad "T6 recorder зовёт replay_digest — самопроверка залезла в горячий путь сбора
        (recorder держит поток на ~3% CPU; ронять это ради самопроверки запрещено)"
  else
    ok "T6 recorder дайджест не считает (самопроверка — отдельный операторский прогон)"
  fi
else
  bad "T6 не найден каталог $REC — канарейка «дайджест не в recorder'е» НЕ проверена"
fi
RDB="$(fn_body "$BIN" 'fn run_replay_digest')"
if [ -z "$RDB" ]; then
  bad "T6 канарейка не смогла извлечь тело run_replay_digest — гейт НЕ проверен"
elif printf '%s' "$RDB" | grep -qE 'read_all|recover\('; then
  bad "T6 режим дайджеста читает журнал через read_all/recover — весь журнал в RAM.
      На проде это 26 GB / 148 млн событий, класс TD-011 (recorder уже переставал писать)"
else
  ok "T6 режим дайджеста потоковый (без read_all/recover)"
fi

# ══════════════════════════════════════════════════════════════════════════════════════
# Задача 7 — запреты milestone'а (что НЕЛЬЗЯ трогать)
# ══════════════════════════════════════════════════════════════════════════════════════
if git diff --name-only origin/main...HEAD 2>/dev/null | grep -q '^crates/contracts/'; then
  bad "T7 тронут crates/contracts — T1 в этом milestone'е НЕ меняется (contract-RFC не открыт)"
else
  ok "T7 crates/contracts не тронут"
fi
if [ "$(count 'const READABLE_SCAN_MAX_CARRY' "$SEG")" -eq 0 ]; then
  bad "T7 READABLE_SCAN_MAX_CARRY исчезла — это граница ПАМЯТИ, несущая для op_8(3);
      снимать её как «решение» проблемы ВРЕМЕНИ запрещено (rev5 M-49 + JR-I-9)"
else
  CARRY=$(grep -m1 'const READABLE_SCAN_MAX_CARRY' "$SEG")
  if printf '%s' "$CARRY" | grep -q '64 \* 1024;'; then
    ok "T7 READABLE_SCAN_MAX_CARRY на месте и не поднята (64 KiB)"
  else
    bad "T7 READABLE_SCAN_MAX_CARRY изменена ($CARRY) — запрещённый размен «терпимость
        против памяти»"
  fi
fi
if [ "$(count 'const FRAME_LEN_SANITY_CAP' "$SEG")" -eq 0 ]; then
  bad "T7 FRAME_LEN_SANITY_CAP исчезла — единая константа санити-капа (JR-I-9, задача 1 M-50)"
else
  ok "T7 FRAME_LEN_SANITY_CAP на месте (JR-I-9 не разобран)"
fi

# ══════════════════════════════════════════════════════════════════════════════════════
# Задача 8 — оракулы: RED-набор M-52 GREEN, наборы M-49/M-50/M-51 БЕЗ ПРАВОК
# ══════════════════════════════════════════════════════════════════════════════════════
echo
echo "--- оракулы M-52 (release: ресурсные границы меряются на оптимизированной сборке) ---"
for t in red_floor_work_budget red_stitch_monotonic red_replay_digest_delivery red_m52_prodscale; do
  if cargo test -p journal --release --test "$t" >/tmp/m52_"$t".log 2>&1; then
    ok "T8 $t GREEN ($(grep -m1 '^test result' /tmp/m52_"$t".log))"
  else
    bad "T8 $t FAILED:
$(grep -E '^(test result|thread|JR-I|assertion|setup-guard|КЛАСС)' /tmp/m52_"$t".log | head -20)"
  fi
done

echo
echo "--- регресс: оракулы M-49/M-50/M-51 обязаны пройти БЕЗ ЕДИНОЙ ПРАВКИ ---"
if git diff --name-only origin/main...HEAD 2>/dev/null \
   | grep -E '^crates/journal/tests/(red_tail_integrity|red_floor_scan|red_det_)' | grep -q .; then
  bad "T8 тронуты sacred-оракулы предыдущих milestone'ов — противоречие с M-49/M-50/M-51
      есть дефект проектирования M-52, а не повод править их тесты"
else
  ok "T8 оракулы M-49/M-50/M-51 не тронуты"
fi
if cargo test -p journal --release >/tmp/m52_journal.log 2>&1; then
  ok "T8 весь крейт journal GREEN ($(grep -c '^test result: ok' /tmp/m52_journal.log) блоков)"
else
  bad "T8 регресс крейта journal:
$(grep -E '^(test result: FAILED|thread|failures:)' /tmp/m52_journal.log | head -20)"
fi
# gateway — ПОТРЕБИТЕЛЬ segments()/stream: именно там прототип вскрыл второй carve-out
# (пустой сегмент). Регресс здесь обязателен наравне с journal.
if cargo test -p gateway --release >/tmp/m52_gateway.log 2>&1; then
  ok "T8 крейт gateway GREEN (потребитель segments()/stream — guard не сломал сшивку)"
else
  bad "T8 регресс крейта gateway (guard монотонности задел потребителя чтения):
$(grep -E '^(test result: FAILED|thread|failures:)' /tmp/m52_gateway.log | head -20)"
fi

# ══════════════════════════════════════════════════════════════════════════════════════
# Задача 9 — CI-паритет
# ══════════════════════════════════════════════════════════════════════════════════════
echo
echo "--- CI-паритет (fmt --all / clippy --workspace --all-targets) ---"
if cargo fmt --all -- --check >/tmp/m52_fmt.log 2>&1; then
  ok "T9 cargo fmt --all -- --check чисто"
else
  bad "T9 cargo fmt --all -- --check:
$(tail -10 /tmp/m52_fmt.log)"
fi
if cargo clippy --workspace --all-targets -- -D warnings >/tmp/m52_clippy.log 2>&1; then
  ok "T9 cargo clippy --workspace --all-targets -D warnings чисто"
else
  bad "T9 clippy:
$(grep -E '^(error|warning)' /tmp/m52_clippy.log | head -15)"
fi

echo
if [ "$FAILS" -eq 0 ]; then
  echo "VERDICT: PASS"
  exit 0
else
  echo "VERDICT: FAIL ($FAILS проверок)"
  exit 1
fi
