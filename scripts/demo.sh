#!/usr/bin/env bash
# OA-06 reproducible two-node demo (Option A).
#
# Drives two fully independent node runtimes exclusively through the frozen
# OA-05 CLI, proves the Option A properties with public-ID/count-only
# assertions plus byte-exact bundle comparisons, and never prints key, token,
# or seed material. Requires bash >= 4.4, GNU coreutils (stat -c, fractional
# sleep), python3, and cargo.
#
# Test-only fault-injection hooks (absent in normal runs; they only delay or
# remove work and never bypass an assertion):
#   OA06_DEMO_READY_TIMEOUT_SECS    readiness bound per daemon start (default 15)
#   OA06_DEMO_TEST_SERVE_DELAY_SECS delay before a serve process is exec'd
#   OA06_DEMO_TEST_CRASH_AFTER_READY=node-a|node-b  KILL a daemon once ready
#   OA06_DEMO_KEEP=1                keep the runtime root after success
#   OA06_DEMO_RUNTIME_ROOT=path     absent or empty directory to use as runtime
set -euo pipefail

readonly ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly STAGES=17
readonly POLL_DECISECONDS=10          # 0.1 s poll quantum
readonly TERM_GRACE_DECISECONDS=100   # 10 s TERM grace before KILL

SUCCESS=0
RT=""
ERRLOG="/dev/null"
PIDS=()
declare -A DAEMON_PID

die() {
  printf 'demo: FAIL %s\n' "$*" >&2
  exit 1
}

# ---------------------------------------------------------------------------
# Runtime root: private (0700), fresh secrets only, override for tests.
# ---------------------------------------------------------------------------
setup_runtime() {
  if [[ -n "${OA06_DEMO_RUNTIME_ROOT:-}" ]]; then
    RT="${OA06_DEMO_RUNTIME_ROOT}"
    if [[ -e "$RT" ]]; then
      [[ -d "$RT" ]] || die "runtime root is not a directory: $RT"
      [[ -z "$(find "$RT" -mindepth 1 -print -quit 2>/dev/null)" ]] \
        || die "runtime root is not empty: $RT"
      chmod 700 "$RT" || die "cannot make runtime root private: $RT"
    else
      mkdir -p "$RT" || die "cannot create runtime root: $RT"
      chmod 700 "$RT" || die "cannot make runtime root private: $RT"
    fi
  else
    RT="$(mktemp -d "${TMPDIR:-/tmp}/oa06-demo.XXXXXXXX")" || die "mktemp failed"
  fi
  READY_TIMEOUT_SECS="${OA06_DEMO_READY_TIMEOUT_SECS:-15}"
  [[ "$READY_TIMEOUT_SECS" =~ ^[1-9][0-9]*$ ]] || die "invalid readiness timeout"
  SERVE_DELAY="${OA06_DEMO_TEST_SERVE_DELAY_SECS:-}"
  if [[ -n "$SERVE_DELAY" ]]; then
    [[ "$SERVE_DELAY" =~ ^[1-9][0-9]*$ ]] || die "invalid serve delay"
  fi
}

# A recorded PID is running only while bash still lists it as one of its own
# live jobs. This cannot mistake a kernel-recycled PID for our child.
job_running() {
  local pid
  for pid in $(jobs -rp); do
    [[ "$pid" == "$1" ]] && return 0
  done
  return 1
}

# ---------------------------------------------------------------------------
# Cleanup: TERM recorded child PIDs only, bounded grace, then KILL; preserve
# the runtime on failure, delete it on success unless OA06_DEMO_KEEP=1.
# ---------------------------------------------------------------------------
term_wait_kill_recorded() {
  local pid waited any
  for pid in "${PIDS[@]}"; do
    if job_running "$pid"; then kill -TERM "$pid" 2>/dev/null || true; fi
  done
  waited=0
  while (( waited < TERM_GRACE_DECISECONDS )); do
    any=0
    for pid in "${PIDS[@]}"; do if job_running "$pid"; then any=1; fi; done
    (( any == 0 )) && break
    sleep 0.1
    waited=$(( waited + 1 ))
  done
  for pid in "${PIDS[@]}"; do
    if job_running "$pid"; then kill -KILL "$pid" 2>/dev/null || true; fi
  done
  for pid in "${PIDS[@]}"; do
    wait "$pid" 2>/dev/null || true
  done
}

cleanup() {
  local status=$?
  trap - INT TERM EXIT
  term_wait_kill_recorded
  if [[ -n "$RT" ]]; then
    if (( SUCCESS == 1 )); then
      if [[ "${OA06_DEMO_KEEP:-}" == "1" ]]; then
        printf 'demo: runtime kept at %s\n' "$RT"
      else
        rm -rf "$RT" \
          || printf 'demo: WARN could not remove runtime at %s\n' "$RT" >&2
      fi
    else
      printf 'demo: runtime preserved at %s\n' "$RT" >&2
    fi
  fi
  exit "$status"
}

# ---------------------------------------------------------------------------
# CLI helpers: exactly one canonical JSON document per command on stdout.
# ---------------------------------------------------------------------------
run() {
  local doc
  if ! doc="$("$BIN" "$@" 2>>"$ERRLOG")"; then
    printf 'demo: FAIL CLI command failed: %s\n  output: %s\n' "$*" "${doc:-<none>}" >&2
    exit 1
  fi
  printf '%s\n' "$doc"
}

jsonget() {
  python3 -I -c '
import json, sys
d = json.load(sys.stdin)
for part in sys.argv[1].split("."):
    d = d[part]
if isinstance(d, bool):
    print(str(d).lower())
else:
    print(d)
' "$1" || {
    printf 'demo: FAIL could not parse a CLI JSON document for %s\n' "$1" >&2
    exit 1
  }
}

field() { printf '%s\n' "$1" | jsonget "$2"; }

expect_field() {
  local doc="$1" path="$2" want="$3" got
  got="$(field "$doc" "$path")"
  [[ "$got" == "$want" ]] || die "assertion failed: $path expected $want, got $got"
}

local_ref_count() { field "$(run show refs --db "$1" --context "$context")" result.refs; }
peer_ref_count()  { field "$(run show refs --db "$1" --context "$context" --peer "$2")" result.refs; }
projection_count(){ field "$(run show projection --db "$1" --context "$context" --head "$2")" result.events; }

# ---------------------------------------------------------------------------
# Daemon lifecycle: 127.0.0.1:0, atomic ready file, hard timeout + liveness.
# ---------------------------------------------------------------------------
start_daemon() {
  local node="$1" ready_name="$2"
  local db="$RT/$node.db" token="$RT/$node.token" ready="$RT/$ready_name"
  local log="$RT/$node.log" pid
  rm -f "$ready"
  if [[ -n "$SERVE_DELAY" ]]; then
    local wrapper="$RT/$node-delay.sh"
    cat > "$wrapper" <<WRAP
#!/usr/bin/env bash
trap 'kill "\$sp" 2>/dev/null || true; exit 143' TERM INT
sleep "\$1" &
sp=\$!
wait "\$sp"
shift
exec "\$@"
WRAP
    chmod 700 "$wrapper"
    "$wrapper" "$SERVE_DELAY" "$BIN" serve \
      --db "$db" --token-file "$token" \
      --listen 127.0.0.1:0 --ready-file "$ready" >> "$log" 2>&1 &
  else
    "$BIN" serve \
      --db "$db" --token-file "$token" \
      --listen 127.0.0.1:0 --ready-file "$ready" >> "$log" 2>&1 &
  fi
  pid=$!
  PIDS+=("$pid")
  DAEMON_PID[$node]=$pid
}

wait_ready() {
  local node="$1" pid="$2" ready="$RT/$3" waited=0
  local limit=$(( READY_TIMEOUT_SECS * POLL_DECISECONDS ))
  while [[ ! -s "$ready" ]]; do
    job_running "$pid" || die "daemon $node exited before readiness (see $RT/$node.log)"
    sleep 0.1
    waited=$(( waited + 1 ))
    if (( waited >= limit )); then
      die "daemon $node readiness timeout after ${READY_TIMEOUT_SECS}s"
    fi
  done
}

stop_daemons_gracefully() {
  local pid waited any
  for pid in "${PIDS[@]}"; do
    if job_running "$pid"; then kill -TERM "$pid" 2>/dev/null || true; fi
  done
  waited=0
  while (( waited < TERM_GRACE_DECISECONDS )); do
    any=0
    for pid in "${PIDS[@]}"; do if job_running "$pid"; then any=1; fi; done
    (( any == 0 )) && break
    sleep 0.1
    waited=$(( waited + 1 ))
  done
  for pid in "${PIDS[@]}"; do
    if job_running "$pid"; then kill -KILL "$pid" 2>/dev/null || true; fi
  done
  for pid in "${PIDS[@]}"; do
    wait "$pid" 2>/dev/null || true
  done
  PIDS=()
}

# up_node <node> <ready-name>: start one daemon and wait for readiness; the
# URL is left in the global UP_URL (never command substitution, which would
# orphan the daemon and lose its recorded PID). The frozen embedded engine
# allows exactly one process per database file, so a node's daemon runs only
# while no local CLI command of that node touches that database.
UP_URL=""
up_node() {
  local node="$1" ready_name="$2"
  start_daemon "$node" "$ready_name"
  wait_ready "$node" "${DAEMON_PID[$node]}" "$ready_name"
  UP_URL="http://$(cat "$RT/$ready_name")"
}

# ---------------------------------------------------------------------------
# The seventeen-stage demo.
# ---------------------------------------------------------------------------
main() {
  cd "$ROOT"
  setup_runtime
  ERRLOG="$RT/cli-stderr.log"
  : > "$ERRLOG"

  # Stage 1: build the locked binaries once.
  cargo build --workspace --locked
  BIN="$ROOT/target/debug/contextmesh"
  AGENT="$ROOT/target/debug/demo_agent"
  [[ -x "$BIN" && -x "$AGENT" ]] || die "locked binaries missing after build"
  printf 'stage 01 build: locked binaries present\n'

  # Stage 2: independent A/B keys and tokens from OS entropy.
  local a_author b_author mode secret
  a_author="$(field "$(run key generate --file "$RT/node-a.key")" result.author)"
  b_author="$(field "$(run key generate --file "$RT/node-b.key")" result.author)"
  [[ "$a_author" != "$b_author" ]] || die "key generation produced identical authors"
  run token generate --file "$RT/node-a.token" > /dev/null
  run token generate --file "$RT/node-b.token" > /dev/null
  run key repair-permissions --file "$RT/node-a.key" > /dev/null
  for secret in node-a.key node-b.key node-a.token node-b.token; do
    mode="$(stat -c '%a' "$RT/$secret")"
    [[ "$mode" == "600" ]] || die "secret file not 0600: $secret"
  done
  printf 'stage 02 key generate+token generate+key repair-permissions: two independent 0600 node identities\n'

  # Stage 3: A creates context/genesis and authorizes B (append-only policy).
  local context genesis create_doc
  create_doc="$(run context create --db "$RT/node-a.db" --key-file "$RT/node-a.key" --branch main)"
  context="$(field "$create_doc" result.context)"
  genesis="$(field "$create_doc" result.genesis)"
  run context authorize --db "$RT/node-a.db" --context "$context" --author "$b_author" > /dev/null
  printf 'stage 03 context create+context authorize: A created context/genesis/main and authorized B\n'

  # Stage 4: export the join descriptor; B provisions explicitly from it.
  run bundle export --db "$RT/node-a.db" --context "$context" \
    --head "$genesis" --out "$RT/join-descriptor.json" > /dev/null
  local descriptor
  descriptor="$(python3 -I -c '
import json, sys
d = json.load(open(sys.argv[1]))
assert d["bundle_version"] == 1
events = d["events"]
assert len(events) == 1, "descriptor must carry exactly one event"
event = events[0]
assert event["body"]["parents"] == [], "descriptor event must be zero-parent"
assert event["body"]["kind"] == "context.genesis"
print(d["context"], event["event_id"])
' "$RT/join-descriptor.json")" || die "join descriptor is malformed"
  local desc_context desc_genesis
  desc_context="${descriptor%% *}"
  desc_genesis="${descriptor##* }"
  [[ "$desc_context" == "$context" && "$desc_genesis" == "$genesis" ]] \
    || die "descriptor disagrees with the creating node"
  run context join --db "$RT/node-b.db" --context "$context" \
    --expected-genesis "$genesis" \
    --author "$a_author" --author "$b_author" > /dev/null
  printf 'stage 04 bundle export+context join: B provisioned from the exported descriptor\n'

  # Stage 5: start independent daemons concurrently on ephemeral loopback
  # ports, proving both nodes boot and publish readiness at the same time.
  start_daemon node-a node-a.ready
  start_daemon node-b node-b.ready
  wait_ready node-a "${DAEMON_PID[node-a]}" node-a.ready
  wait_ready node-b "${DAEMON_PID[node-b]}" node-b.ready
  [[ "$(cat "$RT/node-a.ready")" != "$(cat "$RT/node-b.ready")" ]] \
    || die "daemons published the same address"
  printf 'stage 05 serve: A and B booted on distinct ephemeral loopback ports\n'
  if [[ "${OA06_DEMO_TEST_CRASH_AFTER_READY:-}" == "node-a" \
     || "${OA06_DEMO_TEST_CRASH_AFTER_READY:-}" == "node-b" ]]; then
    local crashed="${OA06_DEMO_TEST_CRASH_AFTER_READY}"
    kill -KILL "${DAEMON_PID[$crashed]}" 2>/dev/null || true
    die "injected crash: $crashed daemon killed after readiness"
  fi
  # The frozen embedded engine allows exactly one process per database file,
  # so daemons are choreographed: each node's daemon runs only while no local
  # CLI command of that node needs its database.
  stop_daemons_gracefully

  # Stage 6: B pulls A's genesis; no implicit local movement.
  local doc url_a
  up_node node-a node-a.ready
  url_a="$UP_URL"
  doc="$(run sync --db "$RT/node-b.db" --peer node-a --url "$url_a" \
    --token-file "$RT/node-a.token" --context "$context")"
  expect_field "$doc" result.inserted 1
  expect_field "$doc" result.remote_refs_updated 1
  [[ "$(field "$doc" result.pages)" -ge 1 ]] || die "pull reported zero pages"
  stop_daemons_gracefully
  [[ "$(local_ref_count "$RT/node-b.db")" == 0 ]] \
    || die "pull moved a B local ref implicitly"
  printf 'stage 06 sync+show refs: B imported A genesis and remote ref only\n'

  # Stage 7: B explicitly creates its local main at genesis.
  run branch create --db "$RT/node-b.db" --context "$context" \
    --name main --from-head "$genesis" > /dev/null
  [[ "$(local_ref_count "$RT/node-b.db")" == 1 ]] || die "B main branch missing"
  printf 'stage 07 branch create: B created local main explicitly\n'

  # Stage 8: A records a distinct request/response chain.
  local a_main a_request
  printf '{"demo":{"echo":"node-a"}}\n' > "$RT/node-a-input.json"
  doc="$(run invoke --db "$RT/node-a.db" --key-file "$RT/node-a.key" \
    --context "$context" --branch main --expected-head "$genesis" \
    --input-file "$RT/node-a-input.json" --provider-command "$AGENT")"
  expect_field "$doc" result.outcome response
  a_main="$(field "$doc" result.result)"
  a_request="$(field "$doc" result.request)"
  expect_field "$(run invocation pending --db "$RT/node-a.db" \
    --context "$context" --branch main)" result.pending 0
  expect_field "$(run invocation detached --db "$RT/node-a.db" \
    --context "$context" --branch main)" result.detached 0
  expect_field "$(run show event --db "$RT/node-a.db" --id "$a_main")" \
    result.kind agent.response
  [[ "$(local_ref_count "$RT/node-a.db")" == 1 ]] || die "A ref count changed"
  printf 'stage 08 invoke+invocation pending+invocation detached+show event: A recorded a linked chain\n'

  # Stage 9: B records its own distinct chain.
  local b_main b_request
  printf '{"demo":{"echo":"node-b"}}\n' > "$RT/node-b-input.json"
  doc="$(run invoke --db "$RT/node-b.db" --key-file "$RT/node-b.key" \
    --context "$context" --branch main --expected-head "$genesis" \
    --input-file "$RT/node-b-input.json" --provider-command "$AGENT")"
  expect_field "$doc" result.outcome response
  b_main="$(field "$doc" result.result)"
  b_request="$(field "$doc" result.request)"
  [[ "$b_main" != "$a_main" && "$b_request" != "$a_request" ]] \
    || die "node chains are not distinct"
  expect_field "$(run invocation pending --db "$RT/node-b.db" \
    --context "$context" --branch main)" result.pending 0
  expect_field "$(run invocation detached --db "$RT/node-b.db" \
    --context "$context" --branch main)" result.detached 0
  [[ "$(local_ref_count "$RT/node-b.db")" == 1 ]] || die "B ref count changed"
  printf 'stage 09 invoke+invocation pending+invocation detached: B recorded a distinct chain\n'

  # Stage 10: pull both directions; local mains stay, peer mains namespace.
  up_node node-a node-a.ready
  doc="$(run sync --db "$RT/node-b.db" --peer node-a --url "$UP_URL" \
    --token-file "$RT/node-a.token" --context "$context")"
  expect_field "$doc" result.inserted 2
  stop_daemons_gracefully
  up_node node-b node-b.ready
  doc="$(run sync --db "$RT/node-a.db" --peer node-b --url "$UP_URL" \
    --token-file "$RT/node-b.token" --context "$context")"
  expect_field "$doc" result.inserted 2
  stop_daemons_gracefully
  [[ "$(local_ref_count "$RT/node-a.db")" == 1 ]] || die "A local main moved"
  [[ "$(local_ref_count "$RT/node-b.db")" == 1 ]] || die "B local main moved"
  [[ "$(peer_ref_count "$RT/node-a.db" node-b)" == 1 ]] || die "A peer ref missing"
  [[ "$(peer_ref_count "$RT/node-b.db" node-a)" == 1 ]] || die "B peer ref missing"
  printf 'stage 10 sync+show refs: both directions exchanged with ref isolation\n'

  # Stage 11: A explicitly merges A-local and remote-B parents by CAS.
  printf '{"demo":{"merge":"node-a-node-b"}}\n' > "$RT/merge.json"
  run branch create --db "$RT/node-a.db" --context "$context" \
    --name merged --from-head "$a_main" > /dev/null
  doc="$(run merge --db "$RT/node-a.db" --key-file "$RT/node-a.key" \
    --context "$context" --branch merged --expected-head "$a_main" \
    --parent "$a_main" --parent "$b_main" --payload-file "$RT/merge.json")"
  local merged
  merged="$(field "$doc" result.event)"
  [[ "$(local_ref_count "$RT/node-a.db")" == 2 ]] || die "A merged ref missing"
  printf 'stage 11 branch create+merge: A merged A-local and remote-B parents\n'

  # Stage 12: B pulls the merge; projections and exported sequences agree and
  # each ancestor appears exactly once.
  up_node node-a node-a.ready
  doc="$(run sync --db "$RT/node-b.db" --peer node-a --url "$UP_URL" \
    --token-file "$RT/node-a.token" --context "$context")"
  expect_field "$doc" result.inserted 1
  stop_daemons_gracefully
  local a_projection b_projection
  a_projection="$(projection_count "$RT/node-a.db" "$merged")"
  b_projection="$(projection_count "$RT/node-b.db" "$merged")"
  [[ "$a_projection" == 6 && "$b_projection" == 6 ]] \
    || die "projection counts disagree: A=$a_projection B=$b_projection"
  run bundle export --db "$RT/node-a.db" --context "$context" \
    --head "$merged" --out "$RT/a-merged-bundle.json" > /dev/null
  run bundle export --db "$RT/node-b.db" --context "$context" \
    --head "$merged" --out "$RT/b-merged-bundle.json" > /dev/null
  python3 -I -c '
import json, sys
a = json.load(open(sys.argv[1]))
b = json.load(open(sys.argv[2]))
assert a["events"] == b["events"], "exported event sequences differ"
assert len(a["events"]) == 6, "exported sequence must be exactly six events"
assert len({e["event_id"] for e in a["events"]}) == 6, "duplicate ancestor"
' "$RT/a-merged-bundle.json" "$RT/b-merged-bundle.json" \
    || die "merged exports disagree between nodes"
  printf 'stage 12 sync+show projection+bundle export: identical 6-event sequences on both nodes\n'

  # Stage 13: restart both daemons on the same databases, new ephemeral ports.
  local old_pids=("${PIDS[@]}")
  stop_daemons_gracefully
  local old_pid
  for old_pid in "${old_pids[@]}"; do
    job_running "$old_pid" && die "old daemon $old_pid survived shutdown"
  done
  up_node node-a node-a-2.ready
  up_node node-b node-b-2.ready
  [[ "$(cat "$RT/node-a-2.ready")" != "$(cat "$RT/node-b-2.ready")" ]] \
    || die "restarted daemons published the same address"
  printf 'stage 13 serve: both daemons reopened the same databases on ephemeral ports\n'
  stop_daemons_gracefully

  # Stage 14: full verification of both stores; observable state unchanged.
  expect_field "$(run verify --db "$RT/node-a.db")" result.valid true
  expect_field "$(run verify --db "$RT/node-b.db")" result.valid true
  [[ "$(projection_count "$RT/node-a.db" "$merged")" == 6 ]] || die "A projection changed"
  [[ "$(projection_count "$RT/node-b.db" "$merged")" == 6 ]] || die "B projection changed"
  [[ "$(peer_ref_count "$RT/node-a.db" node-b)" == 1 ]] || die "A peer refs changed after restart"
  [[ "$(peer_ref_count "$RT/node-b.db" node-a)" == 2 ]] \
    || die "B peer refs changed after restart"
  printf 'stage 14 verify+show projection+show refs: both stores fully valid after restart\n'

  # Stage 15: repeated pulls stream pages, import zero, and move nothing.
  local url_a2 url_b2
  up_node node-b node-b-2.ready
  url_b2="$UP_URL"
  doc="$(run sync --db "$RT/node-a.db" --peer node-b --url "$url_b2" \
    --token-file "$RT/node-b.token" --context "$context")"
  expect_field "$doc" result.inserted 0
  expect_field "$doc" result.remote_refs_updated 0
  [[ "$(field "$doc" result.pages)" -ge 1 ]] || die "A repeat pull skipped the peer"
  stop_daemons_gracefully
  up_node node-a node-a-2.ready
  url_a2="$UP_URL"
  doc="$(run sync --db "$RT/node-b.db" --peer node-a --url "$url_a2" \
    --token-file "$RT/node-a.token" --context "$context")"
  expect_field "$doc" result.inserted 0
  expect_field "$doc" result.remote_refs_updated 0
  [[ "$(field "$doc" result.pages)" -ge 1 ]] || die "B repeat pull skipped the peer"
  stop_daemons_gracefully
  [[ "$(local_ref_count "$RT/node-a.db")" == 2 ]] || die "A refs changed on idempotent pull"
  [[ "$(local_ref_count "$RT/node-b.db")" == 1 ]] || die "B refs changed on idempotent pull"
  printf 'stage 15 sync+show refs: repeat pulls streamed pages, inserted 0, moved no refs\n'

  # Stage 16: a tampered signature is rejected atomically with the frozen
  # failure class and no state change.
  run bundle export --db "$RT/node-b.db" --context "$context" \
    --head "$b_main" --out "$RT/tamper-base.json" > /dev/null
  python3 -I -c '
import sys
src, dst = sys.argv[1], sys.argv[2]
data = open(src, "rb").read()
marker = b"\"signature\":\"sig1_"
cut = data.rfind(marker)          # the last event in parent-first order
assert cut != -1, "no signature field found"
at = cut + len(marker)
original = data[at:at + 1]
for replacement in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_":
    if bytes([replacement]) != original:
        break
else:
    raise SystemExit("no distinct signature byte available")
mutated = data[:at] + bytes([replacement]) + data[at + 1:]
diff = sum(1 for x, y in zip(data, mutated) if x != y)
assert diff == 1 and len(data) == len(mutated), "mutation is not exactly one byte"
open(dst, "wb").write(mutated)
' "$RT/tamper-base.json" "$RT/tampered.json" || die "tamper fixture could not be built"
  local tamper_code tamper_doc
  set +e
  tamper_doc="$("$BIN" bundle import --db "$RT/node-a.db" \
    --peer tamper-node --file "$RT/tampered.json" 2>>"$ERRLOG")"
  tamper_code=$?
  set -e
  [[ $tamper_code -eq 9 ]] || die "tampered import exit was $tamper_code, not the frozen class 9"
  [[ "$(printf '%s\n' "$tamper_doc" | jsonget error.code)" == "internal" ]] \
    || die "tampered import did not report the frozen internal code"
  [[ "$(printf '%s\n' "$tamper_doc" | jsonget ok)" == "false" ]] \
    || die "tampered import did not report failure JSON"
  expect_field "$(run verify --db "$RT/node-a.db")" result.valid true
  [[ "$(local_ref_count "$RT/node-a.db")" == 2 ]] || die "tampered import changed refs"
  [[ "$(peer_ref_count "$RT/node-a.db" tamper-node)" == 0 ]] \
    || die "tampered import created peer refs"
  [[ "$(peer_ref_count "$RT/node-a.db" node-b)" == 1 ]] \
    || die "tampered import changed existing peer refs"
  [[ "$(projection_count "$RT/node-a.db" "$merged")" == 6 ]] \
    || die "tampered import changed the projection"
  printf 'stage 16 bundle export+bundle import: mutated signature rejected with no state change\n'

  # Stage 17: public-ID/count-only PASS summary.
  SUCCESS=1
  printf 'stage 17 summary: public-ID/count-only result below\n'
  printf 'demo: PASS context=%s authors=2 events=6 stages=%d a-refs=2 b-refs=1\n' \
    "$context" "$STAGES"
}

trap 'cleanup' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
main "$@"
