#!/usr/bin/env bash
# A stand-in for the Codex CLI used by Guru's tests. It speaks the command-line
# contract Guru relies on (`--version`, `login status`,
# `exec --json -C DIR -o FILE [--output-schema FILE] -`) and answers with canned,
# schema-shaped JSON chosen by the output file's label. The demo course in
# ui/demo/incident-response provides realistic module and simulation bodies.
set -euo pipefail

DEMO="${FAKE_DEMO_DIR:?FAKE_DEMO_DIR must point at ui/demo/incident-response}"

if [[ "${1:-}" == "--version" ]]; then
  echo "codex-cli 0.0.0-fake"
  exit 0
fi
if [[ "${1:-}" == "login" && "${2:-}" == "status" ]]; then
  echo "Logged in using ChatGPT" >&2
  exit 0
fi
[[ "${1:-}" == "exec" ]] || { echo "fake codex: unsupported: $*" >&2; exit 2; }
shift

out="" cwd="." schema=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    -C) cwd="$2"; shift 2 ;;
    --output-schema) schema="$2"; shift 2 ;;
    --sandbox|-c|-m) shift 2 ;;
    *) shift ;;
  esac
done
prompt="$(cat)"
[[ -n "$out" ]] || { echo "missing -o" >&2; exit 2; }
mkdir -p "$cwd/.guru"
printf '%s\n----\n' "$prompt" >> "$cwd/.guru/fake-prompts.log"

label="$(basename "$out" .txt)"
label="${label#out-}"
echo '{"type":"thread.started","thread_id":"fake"}'
echo '{"type":"item.started","item":{"id":"w1","type":"web_search","query":"current best practices"}}'

# Fail-once helper: the first call for a label misbehaves, later calls succeed.
first_time() {
  local marker="$cwd/.guru/fake-once-$1"
  if [[ -e "$marker" ]]; then return 1; fi
  : > "$marker"
  return 0
}

case "$label" in
  module-m1-0)
    if first_time m1; then
      echo '{"type":"error","message":"stream disconnected before completion"}'
      exit 1
    fi
    cp "$DEMO/modules/m1.json" "$out"
    ;;
  module-m2-0)
    # Too thin to be worth the learner's time: must be sent back with feedback.
    echo '{"lesson":"## Intro\nToo short.","key_points":[],"cards":[],"quiz":[]}' > "$out"
    ;;
  sim-m1s1-0)
    if first_time m1s1; then
      exec sleep 30   # hangs: Guru must time out and retry
    fi
    cp "$DEMO/sims/infected-web-01.json" "$out"
    ;;
  clarify)
    cat > "$out" <<'JSON'
{"restated_goal":"Work safely as an electrician's mate on UK domestic installations","field":"Electrical installation (UK domestic)",
 "questions":[{"id":"x","question":"What is your experience with electrical work?","why":"Sets the starting stage","options":["None","DIY","Apprentice"]},
              {"id":"y","question":"Which country's regulations?","why":"Codes differ","options":["UK (BS 7671)","US (NEC)","Other"]}]}
JSON
    ;;
  goal-plan-*)
    cat > "$out" <<'JSON'
{"title":"Electrician's mate (UK domestic)","field":"Electrical installation","destination":"Assist on domestic installs: safe isolation, testing, fault finding",
 "standards":["BS 7671:2018+A2","GS38"],"tools":["Two-pole tester","Multifunction tester"],"safety":["Safe isolation before any work"],
 "beyond_guru":["Level 3 qualification and supervised site experience"],
 "stages":[{"id":"a","title":"Safe isolation","why":"Nothing else is safe without it","level":30,"hours":2,"outcomes":["Prove dead correctly"],"competencies":["GS38"],"practice":["Lock-off sim"],"assessment":"Isolation sim"},
           {"id":"b","title":"Fault finding","why":"Most call-outs","level":55,"hours":3,"outcomes":["Find RCD faults"],"competencies":["IR testing"],"practice":["RCD sim"],"assessment":"Capstone"},
           {"id":"c","title":"Testing and inspection","why":"Certification","level":80,"hours":4,"outcomes":["Run EICR tests"],"competencies":["MFT"],"practice":["EICR sim"],"assessment":"Report"}]}
JSON
    ;;
  scope)
    echo '{"title":"Widgets Fast","breadth":3,"beginner":{"hours":1,"can_do":"basics"},"intermediate":{"hours":2.5,"can_do":"real work"},"advanced":{"hours":5,"can_do":"expert"},"simulated_tools":["Widget Studio"],"note":"small tool"}' > "$out"
    ;;
  plan)
    cat > "$out" <<'JSON'
{"title":"Widgets Fast","summary":"A fast course.","target_performance":"Ship widgets","out_of_scope":["Gizmos"],
 "skills":[{"id":"triage","name":"Triage","why":"w","weight":9},{"id":"promql","name":"PromQL","why":"w","weight":7}],
 "modules":[
  {"id":"m1","title":"Basics","minutes":45,"summary":"s","objectives":["o"],"skill_ids":["triage"],
   "sims":[{"id":"m1s1","title":"First incident","kind":"incident","tool":"Linux","premise":"p"}]},
  {"id":"m2","title":"Metrics","minutes":45,"summary":"s","objectives":["o"],"skill_ids":["promql"],
   "sims":[{"id":"m2s1","title":"Dash","kind":"tool","tool":"Grafana","premise":"p"}]}],
 "capstone":{"id":"whatever","title":"Final","kind":"incident","tool":"Linux","premise":"p"},
 "research":{"stackexchange_site":"","tags":[],"queries":[]}}
JSON
    ;;
  module-*)
    cp "$DEMO/modules/m1.json" "$out"
    ;;
  sim-m2s1-*)
    if [[ "$prompt" == *"FAILED VALIDATION"* ]]; then
      cp "$DEMO/sims/grafana-alerting.json" "$out"
    else
      # First attempt: an app on a node that does not exist.
      sed 's/"node_id": "graf"/"node_id": "ghost"/' "$DEMO/sims/grafana-alerting.json" > "$out"
    fi
    ;;
  sim-*)
    cp "$DEMO/sims/infected-web-01.json" "$out"
    ;;
  ask)
    echo "Use rate() on counters." > "$out"
    ;;
  *)
    echo "fake codex: unknown label $label" >&2
    exit 3
    ;;
esac
echo '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1,"reasoning_output_tokens":0}}'
