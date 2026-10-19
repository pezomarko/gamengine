#!/usr/bin/env bash
# Smithy gate (docs/ITEMS.md 6.6): the smith's shop and the forge.
#
#   scripts/check-smithy.sh            the two screens against a scripted hub (whole at every
#                                      window size), the town's smithy in the map, the fee;
#                                      with GM_TEST_DATABASE_URL also the hub (the smith sells
#                                      with the coin burned, the forge charges the content's
#                                      fee and refuses another, the ledger balances)
#   scripts/check-smithy.sh --desktop  also the desktop client on a display of its own (Xvfb
#                                      at 1080p, the software GPU): a new character is given
#                                      coin and stood by the smith; by UI script it opens his
#                                      wares, buys iron and oak; stood at the forge, it opens
#                                      the forge, puts the two parts in, reads what they make
#                                      and the fee, and forges a sword; then finds the sword
#                                      in the inventory. A 1080p capture at every step
#                                      (KEEP=DIR keeps them).
# --desktop needs GM_TEST_DATABASE_URL (a Postgres this run wipes).
# Environment: SKIP_BUILD=1, SKIP_TESTS=1, KEEP=DIR (keep logs and screenshots there).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
DESKTOP=0
for a in "$@"; do
  case "$a" in
    --desktop) DESKTOP=1 ;;
    *) echo "check-smithy: unknown argument $a"; exit 2 ;;
  esac
done
status=0
ok()   { echo "OK: $*"; }
fail() { echo "FAIL: $*"; status=1; }
number() { [[ "$1" =~ ^[0-9]+(\.[0-9]+)?$ ]]; }
atleast() {
  if ! number "$1" || ! number "$2"; then fail "$3: no number to check ('$1' against '$2')"
  elif (( ${1%.*} < $2 )); then fail "$3 $1 below $2"; else ok "$3 $1 at least $2"; fi
}
equal() { if [[ "$1" == "$2" ]]; then ok "$3: $1"; else fail "$3: '$1', not '$2'"; fi; }
tests() { # minimum, label, cargo test arguments: the tests must exist and pass
  local min="$1" label="$2" out passed; shift 2
  out="$(cargo test -q "$@" 2>&1)" || { echo "$out" | tail -25; fail "$label"; return; }
  passed="$(echo "$out" | sed -n 's/^test result: ok\. \([0-9]*\) passed.*/\1/p' | sort -n | tail -1)"
  atleast "${passed:-0}" "$min" "$label: tests passed"
}

# 1. Without a display.
if [[ "${SKIP_TESTS:-}" != 1 ]]; then
  tests 3 "the shop and the forge against a scripted hub: the wares by shelf, a buy of the picked one, slots filled by click, what they make asked, both screens whole at every size" -p gm-client smithy::
  tests 1 "the town stands its smith and its forge" -p gm-tools town
  tests 4 "the item content: the fee and the wares among it" -p gm-content items::
  if [[ -n "${GM_TEST_DATABASE_URL:-}" ]]; then
    tests 1 "the hub: the smith sells with the coin burned, the forge charges the content's fee and refuses another, a preview moves nothing" -p gm-hub --test economy the_smith_sells
  else
    echo "note: GM_TEST_DATABASE_URL is not set: the hub's side of the smithy was not run"
  fi
fi
[[ "$DESKTOP" == 1 ]] || exit $status

# 2. A hub and a town.
: "${GM_TEST_DATABASE_URL:?--desktop needs GM_TEST_DATABASE_URL (a Postgres this run wipes)}"
[[ "${SKIP_BUILD:-}" == 1 ]] || cargo build --release -p gm-hub -p gm-server -p gm-client --locked -q
tmp="${KEEP:-$(mktemp -d)}"; mkdir -p "$tmp"; tmp="$(cd "$tmp" && pwd)"; mkdir -p "$tmp/elsewhere"
rm -f "${tmp:?}/hub.der" "${tmp:?}/hub-web.json" "${tmp:?}"/*.log "${tmp:?}/display"
pids=()
cleanup() {
  kill "${pids[@]:-}" 2>/dev/null || true
  [[ -n "${KEEP:-}" ]] || rm -rf "${tmp:?}"
}
trap cleanup EXIT
trap 'exit 130' INT TERM
plain() { sed 's/\x1b\[[0-9;]*m//g' "$1"; }
show() { echo "--- the end of $(basename "$1"):"; plain "$1" 2>/dev/null | tail -"${2:-12}" || true; }
started() { # pid, name, log
  if ! kill -0 "$1" 2>/dev/null; then show "$3"; echo "FAIL: $2 did not start"; exit 1; fi
}
stamp() { # log, text, pid
  for _ in $(seq 1 6000); do
    if /usr/bin/grep -aqF "$2" "$1" 2>/dev/null; then date +%s.%3N; return 0; fi
    kill -0 "$3" 2>/dev/null || return 1
    sleep 0.05
  done
  return 1
}
hp=$((20000 + RANDOM % 20000))
hub=(target/release/gm-hub --database-url "$GM_TEST_DATABASE_URL")
"${hub[@]}" --wipe --listen 127.0.0.1:$hp --cert-out "$tmp/hub.der" \
  --key "$tmp/hub.key" --zone-secret "$tmp" --models-dir "$tmp/models" --auth-per-minute 100000 --start-zone town \
  --web-listen 127.0.0.1:$((hp + 1)) --web-info-out "$tmp/hub-web.json" > "$tmp/hub.log" 2>&1 &
hub_pid=$!; pids+=("$hub_pid")
for _ in $(seq 1 150); do [[ -s "$tmp/hub.der" && -s "$tmp/hub-web.json" ]] && break; kill -0 $hub_pid 2>/dev/null || break; sleep 0.1; done
[[ -s "$tmp/hub.der" && -s "$tmp/hub-web.json" ]] || { show "$tmp/hub.log"; echo "FAIL: the hub did not come up"; exit 1; }
sleep 0.5
link=(--hub 127.0.0.1:$hp --hub-cert "$tmp/hub.der")
target/release/gm-server --map assets/maps/built/town.bsp --listen 127.0.0.1:$((hp + 2)) --cert-out "$tmp/town.der" \
  "${link[@]}" --zone-id town --zone-secret "$tmp" --hz 20 --report-secs 5 \
  --web-listen 127.0.0.1:$((hp + 3)) > "$tmp/town.log" 2>&1 &
town_pid=$!; pids+=("$town_pid")
running=0
for _ in $(seq 1 150); do
  if [[ -s "$tmp/town.der" ]] && /usr/bin/grep -aq "zone running" "$tmp/town.log"; then running=1; break; fi
  kill -0 $town_pid 2>/dev/null || break
  sleep 0.1
done
started $hub_pid "the hub" "$tmp/hub.log"
started $town_pid "the town" "$tmp/town.log"
[[ "$running" == 1 ]] || { show "$tmp/town.log"; echo "FAIL: the town did not come up"; exit 1; }
# Where a buyer stands (town.rs: the smith at 376 -616, the hearth's mouth at 268 -692):
# by the smith, out of the forge's reach; and at the forge, east of the anvil.
BY_SMITH="400,-580,25 225"
AT_FORGE="300,-664,25 200"
PURSE=300
# The operator's hand: a place for a character made by a client, and coin.
place() { # character, spot
  local out="" placed=0
  for _ in $(seq 1 50); do
    # shellcheck disable=SC2086
    if out="$("${hub[@]}" --place "$1" town ${2% *} "${2#* }" 2>&1)"; then placed=1; break; fi
    sleep 0.2
  done
  [[ "$placed" == 1 ]] || { echo "$out" | tail -3; fail "$1 was not placed at $2"; return 1; }
}
quiet() { # log, label
  local first; first="$(/usr/bin/grep -a -m1 '^\[ERROR\]\|^EXCEPTION' "$1" || true)"
  if [[ -z "$first" ]]; then ok "$2: the client logged no error"; else fail "$2: the client logged an error: ${first:0:200}"; fi
}
# A screenshot at every `say shot NAME` of a running script, for a person to look at.
shots() { # log, pid, names...
  local log="$1" pid="$2" n=0; shift 2
  for name in "$@"; do
    if stamp "$log" "ui-script: shot $name" "$pid" > /dev/null; then
      sleep 0.8
      "${on[@]}" xwd -root -silent -out "$tmp/$name.xwd" 2>/dev/null \
        && ffmpeg -loglevel error -y -i "$tmp/$name.xwd" "$tmp/$name.png" 2>/dev/null && rm -f "$tmp/$name.xwd" && n=$((n + 1))
    fi
  done
  echo "$n"
}

desktop() {
  command -v Xvfb >/dev/null || { fail "--desktop needs Xvfb"; return; }
  Xvfb -displayfd 3 -screen 0 1920x1080x24 -nolisten tcp 3> "$tmp/display" > "$tmp/xvfb.log" 2>&1 &
  local xvfb=$!; pids+=("$xvfb")
  for _ in $(seq 1 100); do [[ -s "$tmp/display" ]] && break; kill -0 $xvfb 2>/dev/null || break; sleep 0.1; done
  [[ -s "$tmp/display" ]] || { show "$tmp/xvfb.log"; fail "Xvfb did not start"; return; }
  local disp; disp="$(head -1 "$tmp/display")"
  on=(env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET "DISPLAY=:$disp")
  local client=("$ROOT/target/release/gm-client" --software --size 1920x1080 --settings "$tmp/settings.toml")
  printf 'hub = "127.0.0.1:%s"\nhub_cert = "%s"\nmute = true\n' "$hp" "$tmp/hub.der" > "$tmp/settings.toml"
  local login='wait screen login
expect "email: smithy@gm.test"
field password
type a long password
key Enter
wait screen characters
click Play'

  # 3. A new account and a character, by script; then the operator's hand.
  cat > "$tmp/new.ui" <<'EOS'
wait screen login
click "New account"
field email
type smithy@gm.test
field password
type a long password
field "password again"
type a long password
click "Create account"
wait screen new character
field name
type Crafter
click blade
click Create
wait screen characters
expect "Crafter"
quit
EOS
  if (cd "$tmp/elsewhere" && "${on[@]}" timeout 120 "${client[@]}" --ui-script "$tmp/new.ui") > "$tmp/new.log" 2>&1; then
    ok "a new account and a character called Crafter, by script"
  else
    show "$tmp/new.log"; fail "the crafter was not made"; return
  fi
  place Crafter "$BY_SMITH" || return
  "${hub[@]}" --grant-coin Crafter $PURSE > "$tmp/grant.log" 2>&1 || { show "$tmp/grant.log"; fail "coin for the crafter"; return; }

  # 4. The shop: by the smith, E, iron and oak bought at the prices shown.
  { echo "$login"; cat <<'EOS'
wait screen game
expect "Blacksmith  E talk"
say shot by-the-smith
sleep 2
key E
wait screen smithy
expect "the smith's wares"
expect "3 g"
expect "cores"
expect "gems"
click "iron  edge 30 of 250  15 s"
expect "iron  a core, for crafting"
say shot shop
sleep 2
click Buy
expect "bought iron: it is in the inventory"
expect "2 g 85 s"
click "oak  edge 10 of 250  5 s"
click Buy
expect "bought oak: it is in the inventory"
expect "2 g 80 s"
hover "opal  edge 30 of 250  3 g"
say shot shop-tooltip
sleep 2
click Close
wait screen game
quit
EOS
  } > "$tmp/shop.ui"
  local pid n
  (cd "$tmp/elsewhere" && exec "${on[@]}" timeout 180 "${client[@]}" --ui-script "$tmp/shop.ui") > "$tmp/shop.log" 2>&1 &
  pid=$!; pids+=("$pid")
  n="$(shots "$tmp/shop.log" $pid by-the-smith shop shop-tooltip)"
  if wait $pid && /usr/bin/grep -aq '^ui-script: ok' "$tmp/shop.log"; then
    ok "by script: the smith's wares by the smith, iron and oak bought at the prices shown, the purse 20 silver lighter ($n captures)"
  else
    show "$tmp/shop.log"; fail "the shop by script did not reach its end"; return
  fi
  quiet "$tmp/shop.log" "the desktop client, at the shop"

  # 5. The forge: at the hearth, E, the two parts in, what they make and the fee read,
  #    the sword forged, found in the inventory.
  place Crafter "$AT_FORGE" || return
  { echo "$login"; cat <<'EOS'
wait screen game
expect "Forge  E craft"
say shot at-the-forge
sleep 2
key E
wait screen forge
expect "2 parts in the inventory"
expect "core: nothing"
expect "put a core and a frame in, from the parts beside"
click "iron  a core, for crafting"
expect "core: iron"
expect "a craft needs exactly one core and one frame"
click "oak  a frame, for crafting"
expect "frame: oak"
expect "makes: sword  a weapon, 40 of 250"
expect "20 s"
say shot forge
sleep 2
click Forge
expect "forged: the sword is in the inventory"
expect "2 g 60 s"
expect "core: nothing"
say shot forged
sleep 2
click Close
wait screen game
key I
wait screen inventory
expect "sword  slash +2.0%"
say shot inventory
sleep 2
quit
EOS
  } > "$tmp/forge.ui"
  (cd "$tmp/elsewhere" && exec "${on[@]}" timeout 180 "${client[@]}" --ui-script "$tmp/forge.ui") > "$tmp/forge.log" 2>&1 &
  pid=$!; pids+=("$pid")
  n="$(shots "$tmp/forge.log" $pid at-the-forge forge forged inventory)"
  if wait $pid && /usr/bin/grep -aq '^ui-script: ok' "$tmp/forge.log"; then
    ok "by script: the forge at the hearth, iron and oak put in by a click each, a sword of 40 for a fee of 20 silver forged, in the inventory ($n captures)"
  else
    show "$tmp/forge.log"; fail "the forge by script did not reach its end"; return
  fi
  quiet "$tmp/forge.log" "the desktop client, at the forge"

  # 6. The books: 40 silver burned (15 + 5 + 20), nothing astray.
  local line; line="$("${hub[@]}" --audit 2>/dev/null | /usr/bin/grep -a "^audit:" || true)"
  [[ -n "$line" ]] || { fail "the hub's audit said nothing"; return; }
  echo "$line"
  equal "$(echo "$line" | sed -n 's/.* unsound=\([0-9-]*\).*/\1/p')" 0 "balances that disagree with the ledger"
  equal "$(echo "$line" | sed -n 's/.* burned=\([0-9]*\).*/\1/p')" 40 "silver burned by the smith and the forge"
}
desktop
echo "captures and logs: $tmp"
exit $status
