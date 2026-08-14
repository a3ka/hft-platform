#!/usr/bin/env bash
# Анти-плацебо батарея M-65 (§4.5): эталон + мутанты WS-session.
#
# Предел покрытия назван явно: значение оси 8 `unsubscribe неизвестного id молчит`
# отдельным мутантом НЕ покрыто. Его стережёт `O-10` прямым ассертом; отдельный
# мутант был бы неотличим от `unsubmute` по наблюдаемому профилю. Полнота батареи
# заявляется только относительно таблицы §4.5, а полнота набора — относительно §4.2.
#
# Ключевые правила:
# - kill-set берётся из §4.5 и сверяется РАВЕНСТВОМ множеств;
# - мутанты применяются только в копии дерева;
# - несработавший patch/mutation setup — FAIL батареи;
# - каждый прогон получает собственный CARGO_TARGET_DIR, чтобы не исполнять чужой бинарь.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SPEC="${ROOT}/milestones/M-65-ws-session.md"
TEST_FILE="crates/gateway-serve/tests/red_ws_session.rs"
MODE="${1:---battery}"
PER_TEST_TIMEOUT="${M65_BATTERY_TEST_TIMEOUT:-90}"

case "${MODE}" in
  --battery|--measure) ;;
  *)
    echo "usage: $0 --battery|--measure" >&2
    exit 2
    ;;
esac

FAILED=0
PASSED_MUTANTS=0
declare -a MUTANTS=()
declare -a TESTS=()
declare -A AXIS=()
declare -A EXPECTED=()
declare -A OBSERVED=()

WORK="$(mktemp -d /tmp/m65-ws-battery-XXXXXX)" || {
  echo "SETUP НЕ СОСТОЯЛСЯ: mktemp" >&2
  exit 1
}
cleanup() {
  if [ "${KEEP_M65_BATTERY:-0}" = "1" ]; then
    echo "логи/копии сохранены: ${WORK}" >&2
    return
  fi
  rm -rf "${WORK}"
}
trap cleanup EXIT

pass() { echo "PASS  $*"; }
fail() { echo "FAIL  $*"; FAILED=$((FAILED + 1)); }
die() {
  echo "SETUP НЕ СОСТОЯЛСЯ: $*" >&2
  exit 1
}

trim() {
  sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//'
}

supported_mutant() {
  case "$1" in
    envwins|stalefeed|submerge|capopen|versionmute|harshdrop|lateignore|prosaicerr|connshare|crosstalk|unsubmute|capleak|emptyframe) return 0 ;;
    *) return 1 ;;
  esac
}

extract_tests_from_cell() {
  grep -oE '`[^`]+`' | tr -d '`' | grep -E '^o[0-9]+[A-Za-z0-9_]*$' | sort -u | tr '\n' ' ' | sed -E 's/[[:space:]]+$//'
}

parse_spec_table() {
  [ -f "${SPEC}" ] || die "нет спеки ${SPEC}"
  local in_table=0 line name axis_cell kill_cell tests
  while IFS= read -r line; do
    if [[ "${line}" == "### 4.5"* ]]; then
      in_table=1
      continue
    fi
    if [ "${in_table}" = "1" ] && [[ "${line}" == "### 4.6"* ]]; then
      break
    fi
    [ "${in_table}" = "1" ] || continue
    [[ "${line}" =~ ^\|\ \`([a-z0-9_]+)\` ]] || continue
    name="${BASH_REMATCH[1]}"
    supported_mutant "${name}" || die "§4.5 объявил мутант ${name}, но батарея не знает его patch"
    IFS='|' read -r _ _ axis_cell kill_cell _ <<< "${line}"
    axis_cell="$(printf '%s' "${axis_cell}" | trim)"
    tests="$(printf '%s\n' "${kill_cell}" | extract_tests_from_cell)"
    MUTANTS+=("${name}")
    AXIS["${name}"]="${axis_cell}"
    EXPECTED["${name}"]="${tests}"
    if [ "${MODE}" = "--battery" ] && [ -z "${tests}" ]; then
      die "§4.5: kill-set мутанта ${name} не заполнен замером"
    fi
  done < "${SPEC}"
  [ "${#MUTANTS[@]}" -gt 0 ] || die "§4.5: таблица мутантов не распознана"
}

format_set() {
  local set="${1:-}" out="" t
  if [ -z "${set}" ]; then
    printf '∅'
    return
  fi
  for t in ${set}; do
    if [ -n "${out}" ]; then
      out="${out} · "
    fi
    out="${out}\`${t}\`"
  done
  printf '%s' "${out}"
}

normalize_set() {
  local set="${1:-}"
  if [ -n "${set}" ]; then
    printf '%s\n' ${set} | sed '/^$/d' | sort -u
  fi
}

sets_equal() {
  local expected="${1:-}" observed="${2:-}" diff_file="$3"
  comm -23 <(normalize_set "${expected}") <(normalize_set "${observed}") > "${diff_file}.missing"
  comm -13 <(normalize_set "${expected}") <(normalize_set "${observed}") > "${diff_file}.extra"
  [ ! -s "${diff_file}.missing" ] && [ ! -s "${diff_file}.extra" ]
}

copy_tree() {
  local dest="$1"
  mkdir -p "${dest}" || return 1
  (
    cd "${ROOT}" || exit 1
    git ls-files -z | tar --null -T - -cf -
  ) | (
    cd "${dest}" || exit 1
    tar -xf -
  )
}

replace_once() {
  local file="$1"
  local find="$2"
  local repl="$3"
  M65_FIND="${find}" M65_REPL="${repl}" perl -0pi -e '
    BEGIN {
      $find = $ENV{"M65_FIND"};
      $repl = $ENV{"M65_REPL"};
      $count = 0;
    }
    $count += s/\Q$find\E/$repl/g;
    END {
      if ($count != 1) {
        print STDERR "replacement count=$count\n";
        exit 7;
      }
    }
  ' "${file}"
}

apply_mutant() {
  local name="$1"
  case "${name}" in
    envwins)
      replace_once crates/gateway-serve/src/lib.rs \
        '                let sel_for_resume = sel.clone();' \
        '                let sel_for_resume = inner.cfg.selector.clone();'
      ;;
    stalefeed)
      replace_once crates/gateway-serve/src/lib.rs \
        '                    let old = inner.subs.insert(switched_id.clone(), new_sub);' \
        '                    let old = Some(new_sub);'
      ;;
    submerge)
      replace_once crates/gateway-serve/src/lib.rs \
        '                inner.subs.insert(id_for_insert.clone(), new_sub);' \
        '                let merged_id = inner
                    .subs
                    .keys()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| id_for_insert.clone());
                let mut merged_sub = new_sub;
                merged_sub.id = merged_id.clone();
                inner.subs.insert(merged_id, merged_sub);'
      ;;
    capopen)
      replace_once crates/gateway-serve/src/lib.rs \
        '                if inner.subs_count >= max_subs {' \
        '                if false && inner.subs_count >= max_subs {'
      ;;
    versionmute)
      replace_once crates/gateway-serve/src/wire_v1.rs \
        '    if ver_u64 != 1 {' \
        '    if false && ver_u64 != 1 {'
      ;;
    harshdrop)
      replace_once crates/gateway-serve/src/lib.rs \
        '                    send_v1_error(sink, Some(id), "invalid_selector", &msg).await;
                    return Err(format!("invalid selector: {msg}"));' \
        '                    panic!("harshdrop mutant closes on invalid selector: {msg}");'
      ;;
    lateignore)
      replace_once crates/gateway-serve/src/lib.rs \
        '                    let _ = handle_v1_message(&parsed, inner, sink).await;' \
        '                    let _ = parsed;'
      ;;
    prosaicerr)
      replace_once crates/gateway-serve/src/wire_v1.rs \
        '        "code":code,
' \
        ''
      ;;
    connshare)
      replace_once crates/gateway-serve/src/lib.rs \
        '                // Два пути:' \
        '                let sel = {
                    static SHARED_BY_SUB_ID: std::sync::OnceLock<
                        std::sync::Mutex<std::collections::BTreeMap<String, Selector>>,
                    > = std::sync::OnceLock::new();
                    let mut shared = SHARED_BY_SUB_ID
                        .get_or_init(|| std::sync::Mutex::new(std::collections::BTreeMap::new()))
                        .lock()
                        .expect("connshare mutex poisoned");
                    shared.entry(id.clone()).or_insert_with(|| sel.clone()).clone()
                };
                // Два пути:'
      ;;
    crosstalk)
      replace_once crates/gateway-serve/src/lib.rs \
        '    pub fn effective_max_subs() -> usize {
        EFFECTIVE_MAX_SUBS.load(Ordering::Relaxed)
    }
' \
        '    pub fn effective_max_subs() -> usize {
        EFFECTIVE_MAX_SUBS.load(Ordering::Relaxed)
    }
    static CROSSTALK_TO_ETH: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
'
      replace_once crates/gateway-serve/src/lib.rs \
        '                // Два пути:' \
        '                if id == "w" && sel.symbol == "ETHUSDT" {
                    CROSSTALK_TO_ETH.store(true, Ordering::Relaxed);
                }
                // Два пути:'
      replace_once crates/gateway-serve/src/lib.rs \
        '                        let cfg2 = Arc::clone(&inner.cfg);' \
        '                        if CROSSTALK_TO_ETH.load(Ordering::Relaxed)
                            && sub.id == "w"
                            && sub.selector.symbol == "BTCUSDT"
                        {
                            sub.selector.symbol = "ETHUSDT".to_string();
                            let ckpt_dir = inner.cfg.checkpoint_dir.clone().unwrap_or_default();
                            if let Ok((live, _)) = crate::_gw::LiveReducer::resume(
                                inner.cfg.journal_dir.as_path(),
                                inner.cfg.filter.clone(),
                                &sub.selector,
                                ckpt_dir.as_path(),
                            ) {
                                sub.live = live;
                            }
                        }
                        let cfg2 = Arc::clone(&inner.cfg);'
      ;;
    unsubmute)
      replace_once crates/gateway-serve/src/lib.rs \
        '                let id_str = id.clone();' \
        '                let id_str = id.clone();
                let _ = id_str;
                return Ok(());'
      ;;
    capleak)
      replace_once crates/gateway-serve/src/lib.rs \
        '                inner.subs_count = inner.subs_count.saturating_sub(1);' \
        '                inner.subs_count = inner.subs_count;'
      ;;
    emptyframe)
      replace_once crates/gateway-serve/src/lib.rs \
        '                        Ok((sub, frames, _new_cursor, _gen_at_pump, _stats)) => {' \
        '                        Ok((sub, mut frames, new_cursor, _gen_at_pump, _stats)) => {'
      replace_once crates/gateway-serve/src/lib.rs \
        '                            // heartbeat-кадр.
                            for frame in frames {' \
        '                            // heartbeat-кадр.
                            if frames.is_empty() {
                                frames.push(crate::_gw::Frame {
                                    schema_version: crate::_gw::GATEWAY_SCHEMA_VERSION,
                                    from: new_cursor,
                                    to: new_cursor,
                                    delta: crate::_gw::SeriesBundle::default(),
                                    at_ms: 0,
                                });
                            }
                            for frame in frames {'
      ;;
    *)
      return 2
      ;;
  esac
}

build_test_list_and_reference() {
  local tree="${WORK}/reference"
  local target="${WORK}/target-reference"
  local list_log="${WORK}/reference-list.log"
  local ref_log="${WORK}/reference.log"
  copy_tree "${tree}" || die "не удалось скопировать дерево для эталона"

  if ! (cd "${tree}" && CARGO_TARGET_DIR="${target}" cargo test -p gateway-serve --test red_ws_session -- --list > "${list_log}" 2>&1); then
    fail "эталон: список тестов не получен"
    tail -20 "${list_log}" | sed 's/^/      ↳ /'
    return 1
  fi
  mapfile -t TESTS < <(awk -F: '/: test$/ {print $1}' "${list_log}" | sort)
  if [ "${#TESTS[@]}" -eq 0 ]; then
    fail "эталон: список тестов пуст — батарея не исполняет оракулы"
    return 1
  fi

  if (cd "${tree}" && CARGO_TARGET_DIR="${target}" cargo test -p gateway-serve --test red_ws_session -- --test-threads=1 > "${ref_log}" 2>&1); then
    local n_run
    n_run="$(grep -cE '^test .* \.\.\. ok$' "${ref_log}" || true)"
    if [ "${n_run}" -eq "${#TESTS[@]}" ]; then
      pass "эталон GREEN: ${n_run}/${#TESTS[@]} оракулов, CARGO_TARGET_DIR=${target}"
      return 0
    fi
    fail "эталон: cargo test вернул 0, но исполнено ${n_run}/${#TESTS[@]} тестов"
    return 1
  fi

  fail "эталон КРАСНЫЙ"
  grep -E '^test .* FAILED|^---- ' "${ref_log}" | head -20 | sed 's/^/      ↳ /'
  return 1
}

run_one_mutant() {
  local name="$1"
  local tree="${WORK}/mutant-${name}"
  local target="${WORK}/target-${name}"
  local patch_log="${WORK}/${name}-patch.log"
  local build_log="${WORK}/${name}-build.log"
  local observed="" test_name log rc ran

  echo "RUN   ${name}: ${AXIS[${name}]} · CARGO_TARGET_DIR=${target}"
  copy_tree "${tree}" || {
    fail "${name}: копия дерева не создана"
    return 1
  }
  if ! (cd "${tree}" && apply_mutant "${name}" > "${patch_log}" 2>&1); then
    fail "${name}: мутация НЕ ПРИМЕНИЛАСЬ — setup fail-closed"
    tail -20 "${patch_log}" | sed 's/^/      ↳ /'
    return 1
  fi
  if ! (cd "${tree}" && CARGO_TARGET_DIR="${target}" cargo test -p gateway-serve --test red_ws_session --no-run > "${build_log}" 2>&1); then
    fail "${name}: мутант не компилируется — это не валидный одноосевой мутант"
    tail -20 "${build_log}" | sed 's/^/      ↳ /'
    return 1
  fi

  for test_name in "${TESTS[@]}"; do
    log="${WORK}/${name}-${test_name}.log"
    (cd "${tree}" && CARGO_TARGET_DIR="${target}" timeout "${PER_TEST_TIMEOUT}" cargo test -p gateway-serve --test red_ws_session "${test_name}" -- --exact --test-threads=1 > "${log}" 2>&1)
    rc=$?
    ran="$(grep -c '^running 1 test' "${log}" || true)"
    if [ "${ran}" -ne 1 ]; then
      fail "${name}/${test_name}: тест не был исполнен ровно один раз (rc=${rc})"
      tail -20 "${log}" | sed 's/^/      ↳ /'
      return 1
    fi
    if [ "${rc}" -ne 0 ]; then
      observed="${observed}${test_name} "
    fi
  done
  observed="$(printf '%s' "${observed}" | sed -E 's/[[:space:]]+$//')"
  OBSERVED["${name}"]="${observed}"

  if [ "${MODE}" = "--measure" ]; then
    echo "MEASURE ${name}: $(format_set "${observed}")"
    return 0
  fi

  local diff_base="${WORK}/${name}-setdiff"
  if sets_equal "${EXPECTED[${name}]}" "${observed}" "${diff_base}"; then
    PASSED_MUTANTS=$((PASSED_MUTANTS + 1))
    pass "${name}: kill-set совпал: $(format_set "${observed}")"
    return 0
  fi

  fail "${name}: kill-set НЕ совпал"
  echo "      ожидалось: $(format_set "${EXPECTED[${name}]}")"
  echo "      упало:     $(format_set "${observed}")"
  if [ -s "${diff_base}.missing" ]; then
    echo "      меньше объявленного:"
    sed 's/^/        - /' "${diff_base}.missing"
  fi
  if [ -s "${diff_base}.extra" ]; then
    echo "      больше объявленного:"
    sed 's/^/        - /' "${diff_base}.extra"
  fi
  return 1
}

parse_spec_table

echo "══ M-65 WS-session mutation battery ══"
echo "спека: ${SPEC}"
echo "таблица §4.5: ${#MUTANTS[@]} мутантов"
echo "предел покрытия: ось 8 / \`unsubscribe неизвестного id молчит\` — прямой ассерт O-10, без отдельного мутанта"
echo

build_test_list_and_reference

for mutant in "${MUTANTS[@]}"; do
  run_one_mutant "${mutant}"
done

if [ "${MODE}" = "--measure" ]; then
  echo
  echo "MEASURED KILL-SETS (§4.5):"
  for mutant in "${MUTANTS[@]}"; do
    echo "| \`${mutant}\` | $(format_set "${OBSERVED[${mutant}]:-}") |"
  done
  if [ "${FAILED}" -gt 0 ]; then
    echo "MEASURE: FAIL (${FAILED} setup/runtime нарушений)"
    exit 1
  fi
  echo "MEASURE: PASS (${#MUTANTS[@]}/${#MUTANTS[@]})"
  exit 0
fi

echo
if [ "${FAILED}" -gt 0 ]; then
  echo "BATTERY: FAIL (${PASSED_MUTANTS} из ${#MUTANTS[@]})"
  exit 1
fi
echo "BATTERY: PASS (${#MUTANTS[@]}/${#MUTANTS[@]})"
