#!/usr/bin/env bash
# M-54 §6 — замер стоимости подключения против ПРОДА (acceptance «прогон на проде»).
#
# Milestone закрывается не зелёными тестами, а поведением живого сервера: тесты доказывают,
# что второй проход невозможен по построению, а этот скрипт показывает, во что это обошлось
# пользователю панели.
#
# Базовые точки для сравнения:
#   28 488 / 142 469 / 66 290 ms — до всего (R-026 §7, чекпоинт раз в сутки, двойной расчёт)
#    5 096 /   6 498 / 10 263 ms — после учащения чекпоинта до 15 мин (14017a4), 2026-08-03T17:2xZ
#   ожидание после M-54 — секунды, и разброс уже; двойной расчёт устранён
#
# ЗАПУСК: bash scripts/measure_M-54_connect.sh [runs]
set -uo pipefail

RUNS="${1:-3}"
VPS="${VPS:-root@167.233.192.131}"
KEY="${KEY:-/home/nous/.ssh/hft_deploy}"
SSH=(ssh -i "$KEY" -o IdentitiesOnly=yes -o StrictHostKeyChecking=no "$VPS")

echo "=== M-54: стоимость подключения на проде ($RUNS прогонов) ==="

# ── Почему токен подписывается здесь, а не берётся у wsprobe ────────────────────────────
# TD-095/TD-084: `wsprobe --secret` НЕ подключается к проду. `parse_secret`
# (crates/gateway-serve/src/bin/wsprobe.rs:155) видит строку из одних hex-цифр, считает её
# шестнадцатеричной и декодирует 64 символа в 32 байта; сервер же берёт секрет как ASCII
# (crates/gateway-serve/src/lib.rs:752, DecodingKey::from_secret(secret.as_bytes())).
# Прод-секрет — ровно 64 hex-символа, поэтому ключи расходятся: `invalid token`.
# Пока дефект не закрыт, подписываем HS256 сами и передаём --token.
"${SSH[@]}" 'set -uo pipefail
S=$(docker inspect hft-gateway-serve --format "{{range .Config.Env}}{{println .}}{{end}}" \
    | grep -oP "(?<=^GATEWAY_JWT_SECRET=).*")
[ -n "$S" ] || { echo "FAIL: GATEWAY_JWT_SECRET не найден в env контейнера"; exit 1; }

T=$(python3 -c "
import hmac,hashlib,base64,json,time,sys
s=sys.argv[1].encode()
b=lambda d: base64.urlsafe_b64encode(d).rstrip(b\"=\")
h=b(json.dumps({\"alg\":\"HS256\",\"typ\":\"JWT\"},separators=(\",\",\":\")).encode())
p=b(json.dumps({\"sub\":\"wsprobe\",\"exp\":int(time.time())+3600},separators=(\",\",\":\")).encode())
m=h+b\".\"+p
print((m+b\".\"+b(hmac.new(s,m,hashlib.sha256).digest())).decode())" "$S")

echo "--- отставание чекпоинта от записи (первое слагаемое латентности) ---"
COV=$(cat /var/lib/docker/volumes/hft-platform_gateway-ckpt/_data/covered_through_seq 2>/dev/null || echo 0)
NEXT=$(python3 -c "import json;print(json.load(open(\"/var/lib/docker/volumes/hft-platform_journal-data/_data/recorder.heartbeat\"))[\"next_seq\"])")
echo "covered=$COV next_seq=$NEXT backlog=$((NEXT-COV)) событий; чекпоинт: $(cat /var/lib/hft/gateway-checkpoint.last-success 2>/dev/null)"

echo "--- прогоны ---"
for i in $(seq 1 '"$RUNS"'); do
  docker exec hft-gateway-serve /usr/local/bin/wsprobe \
      --url ws://127.0.0.1:8080 --token "$T" --frames 5 --seconds 25 --out /tmp/m54-$i 2>&1 \
    | grep -oE "latency_first_snapshot_ms=[0-9]+|frames_received=[0-9]+|error.*" | tr "\n" " "
  echo
done

echo "--- состояние сервера ПОСЛЕ прогонов (тихая деградация ловится здесь) ---"
docker stats --no-stream --format "{{.Name}} CPU={{.CPUPerc}} MEM={{.MemUsage}}" hft-gateway-serve hft-recorder
echo "CLOSE_WAIT=$(ss -tan 2>/dev/null | grep -c CLOSE-WAIT)"
docker ps --format "{{.Names}} {{.Status}}"
'
RC=$?
echo
if [ "$RC" -eq 0 ]; then
  echo "Замер снят. Сверь с базовыми точками в шапке скрипта и занеси в close-out M-54."
else
  echo "ЗАМЕР НЕ СНЯТ (exit=$RC) — прод недоступен или изменилась схема; milestone не закрывать."
fi
exit "$RC"
