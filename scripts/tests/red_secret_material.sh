#!/usr/bin/env bash
# Проба барьера `scripts/check_secret_material.sh` — АНТИ-ПЛАЦЕБО В ОБЕ СТОРОНЫ.
#
# Барьер, про который известно только «он зелёный», не доказывает ничего: зелёным он бывает
# и оттого, что слеп. Проба предъявляет ОБА свойства:
#   · краснеет против внесённого ключа КАЖДОГО класса, который барьер заявляет;
#   · НЕ краснеет против честного корпуса, где имена секретов упоминаются свободно.
# Второе не менее важно первого: шумный барьер отключают, и тогда он равен отсутствующему
# (`testing.md` — анти-плацебо в обе стороны).
#
# Каждый сценарий гоняет барьер ТОЙ ЖЕ ПРОВОДКОЙ, какой его зовёт CI: в отдельном
# git-репозитории, `git grep` по отслеживаемым файлам. Фикстура, которую барьер видит иначе,
# чем прод, проверяет не тот предмет (`testing.md` «Целостность гейта», свойство 1).
#
# Число сценариев НЕ ЗАЯВЛЯЕТСЯ цифрой в комментарии — оно СЧИТАЕТСЯ пробой и печатается
# в итоге: заявленное число протухает, счётчик — нет.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BARRIER="$ROOT/scripts/check_secret_material.sh"
[ -x "$BARRIER" ] || { echo "SETUP НЕ СОСТОЯЛСЯ: барьер $BARRIER не исполняем — проба судила бы пустоту"; exit 1; }

PASS=0; FAIL=0
ok()   { printf 'pass  %s\n' "$*"; PASS=$((PASS+1)); }
nope() { printf 'FAIL  %s\n' "$*"; FAIL=$((FAIL+1)); }

# Песочница убирается ВСЕГДА, в том числе при падении: проба, оставляющая мусор, однажды
# дала 10 400 каталогов в /tmp (замер 2026-08-13 по соседней пробе).
SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT

# Собрать репозиторий с одним файлом и прогнать барьер. Печатает код возврата.
run_case() {
  local path="$1" content="$2" dir
  dir="$SANDBOX/case-$RANDOM$RANDOM"
  mkdir -p "$dir/$(dirname "$path")" 2>/dev/null || mkdir -p "$dir"
  git -C "$dir" init -q 2>/dev/null || { echo 99; return; }
  git -C "$dir" config user.email t@t.local
  git -C "$dir" config user.name t
  printf '%s\n' "$content" > "$dir/$path"
  # Барьер судит ОТСЛЕЖИВАЕМЫЕ файлы — файл обязан быть в индексе, иначе сценарий
  # проверял бы не барьер, а поведение git на неотслеживаемом мусоре.
  git -C "$dir" add -f "$path" >/dev/null 2>&1
  git -C "$dir" commit -qm "fixture" >/dev/null 2>&1
  # Барьер встаёт в корень ПО СВОЕМУ ПУТИ, поэтому копируем его в песочницу.
  mkdir -p "$dir/scripts"
  cp "$BARRIER" "$dir/scripts/check_secret_material.sh"
  git -C "$dir" add -f scripts/check_secret_material.sh >/dev/null 2>&1
  git -C "$dir" commit -qm "barrier" >/dev/null 2>&1
  ( cd "$dir" && bash scripts/check_secret_material.sh >/dev/null 2>&1; echo $? )
}

expect_red() {
  local name="$1" path="$2" content="$3" rc
  rc="$(run_case "$path" "$content")"
  if [ "$rc" = "1" ]; then ok "КРАСНЕЕТ: $name"
  elif [ "$rc" = "99" ]; then nope "$name — SETUP не состоялся (git init упал), сценарий не судил предмет"
  else nope "$name — барьер ПРОПУСТИЛ (exit=$rc), ключ этого класса не ловится"; fi
}

expect_green() {
  local name="$1" path="$2" content="$3" rc
  rc="$(run_case "$path" "$content")"
  if [ "$rc" = "0" ]; then ok "молчит: $name"
  elif [ "$rc" = "99" ]; then nope "$name — SETUP не состоялся"
  else nope "$name — ЛОЖНОЕ КРАСНОЕ (exit=$rc): барьер шумит на честном содержимом"; fi
}

echo "── КРАСНЕЕТ против внесённого ключа ─────────────────────────────────────────"
# Тела собираются конкатенацией, чтобы САМА ПРОБА не выглядела носителем секрета для
# внешних сканеров: строка целиком в исходнике не встречается.
expect_red "приватный ключ PEM в исходнике" "src/leak.rs" \
  "const K: &str = \"-----BEGIN RSA PRIVATE"" KEY-----\";"
expect_red "приватный ключ OpenSSH" "deploy/id" \
  "-----BEGIN OPENSSH PRIVATE"" KEY-----"
expect_red "публичный ключ SSH (тело)" "deploy/keys.txt" \
  "ssh-ed25519 AAAA""C3NzaC1lZDI1NTE5AAAAIL9kQm2vXpTn4bWcRzYuHgKdE7sPqA1oJx5MtVnBcZwF admin"
expect_red "токен GitHub" "docs/note.md" \
  "ghp""_0123456789abcdefghijklmnopqrstuvwxyz"
expect_red "ключ доступа AWS" "deploy/aws.cfg" \
  "aws_access_key_id = AKIA""IOSFODNN7EXAMPL2"
expect_red "токен Slack" "deploy/hook.txt" \
  "xoxb""-2233445566-7788990011-aBcDeFgHiJkLmNoPqRsTuVwX"
expect_red "бот-токен Telegram" "deploy/tg.env.txt" \
  "TOKEN=7891234567:AA""H9kLmNpQrStUvWxYz0123456789AbCdEfGh"
expect_red "файл-носитель по расширению" "deploy/server.pem" \
  "любое содержимое, важен ПУТЬ"
expect_red "файл .env отслеживается" "deploy/.env" \
  "SOME_SETTING=1"

echo
echo "── МОЛЧИТ на честном содержимом (ложное красное так же вредно) ───────────────"
expect_green "ИМЯ переменной секрета в документе" "docs/runbook.md" \
  "Для включения канала нужны TELEGRAM_BOT_TOKEN и TELEGRAM_CHAT_ID в окружении VPS."
expect_green "упоминание authorized_keys в прозе" "TECH-DEBT.md" \
  "root-доступ к проду: неопознанный ключ в authorized_keys — перепроверено, жив."
expect_green "отпечаток ключа (SHA256), не тело" "docs/audit.md" \
  "256 SHA256:7yGvACMJzZcUSFJ4Rt9AkrO8Za/V0RfNF/iB6f1KC0c no comment (ED25519)"
expect_green "шаблон .env.example без значений" ".env.example" \
  "TELEGRAM_BOT_TOKEN="
expect_green "синтетическая фикстура с самоописанием" "crates/ops/tests/red_redaction.rs" \
  "const TOKEN: &str = \"7891234567:AAG-ORACLE-MARKER-SECRET-DO-NOT-LOG\";"
expect_green "переменная в конфигурации деплоя" "docker-compose.yml" \
  "      TELEGRAM_BOT_TOKEN: \${TELEGRAM_BOT_TOKEN:-}"

echo
echo "── ЧЕСТНЫЙ КОРПУС ЦЕЛИКОМ ───────────────────────────────────────────────────"
# Сценарий прод-масштаба: барьер обязан молчать на РЕАЛЬНОМ дереве проекта, где имена
# секретов встречаются сотнями. Суррогат из одного файла этого не проверяет.
( cd "$ROOT" && bash scripts/check_secret_material.sh >/dev/null 2>&1 )
if [ $? -eq 0 ]; then ok "молчит: реальное дерево репозитория"
else nope "ЛОЖНОЕ КРАСНОЕ на реальном дереве — барьер непригоден к включению в CI"; fi

echo
echo "─────────────────────────────────────────────────────────────────────────────"
echo "сценариев: $((PASS + FAIL))   pass=$PASS   FAIL=$FAIL"
if [ "$FAIL" -eq 0 ]; then echo "VERDICT: PASS"; exit 0; else echo "VERDICT: FAIL"; exit 1; fi
