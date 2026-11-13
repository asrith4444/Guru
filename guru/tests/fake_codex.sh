#!/usr/bin/env bash
# A stand-in for the Codex CLI used by Guru's end-to-end test. It speaks the
# same command-line contract Guru relies on (`--version`, `login status`,
# `exec --json -C DIR -o FILE --output-schema FILE -`) and returns canned,
# schema-shaped answers keyed by the output file name.
set -euo pipefail

if [[ "${1:-}" == "--version" ]]; then
  echo "codex-cli 0.0.0-fake"
  exit 0
fi
if [[ "${1:-}" == "login" && "${2:-}" == "status" ]]; then
  echo "Logged in using ChatGPT" >&2
  exit 0
fi
if [[ "${1:-}" != "exec" ]]; then
  echo "fake codex: unsupported invocation: $*" >&2
  exit 2
fi
shift

out="" cwd="." schema=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o) out="$2"; shift 2 ;;
    -C) cwd="$2"; shift 2 ;;
    --output-schema) schema="$2"; shift 2 ;;
    --sandbox|-c|-m|--image) shift 2 ;;
    *) shift ;;
  esac
done
prompt="$(cat)"
[[ -n "$out" ]] || { echo "missing -o" >&2; exit 2; }
# Record every prompt so the test can assert on what Guru sends.
mkdir -p "$cwd/.guru"
printf '%s\n----\n' "$prompt" >> "$cwd/.guru/fake-prompts.log"
[[ -n "$schema" ]] && [[ -f "$schema" ]] || [[ "$(basename "$out")" == out-ask.txt ]] || [[ "$(basename "$out")" == out-report.txt ]] || { echo "missing schema" >&2; exit 2; }

label="$(basename "$out" .txt)"
label="${label#out-}"
echo '{"type":"thread.started","thread_id":"fake"}'
echo '{"type":"turn.started"}'
echo '{"type":"item.started","item":{"id":"c1","type":"command_execution","command":"ls","aggregated_output":"","exit_code":null,"status":"in_progress"}}'

case "$label" in
  plan)
    cat > "$out" <<'JSON'
{"title":"Test Topic in 7 Hours","domain":"software","arena":"code_lab",
 "target_performance":"Build and debug a small widget service",
 "out_of_scope":["clustering"],"level_estimate":"beginner",
 "skills":[{"id":"basics","name":"Basics","why_it_matters":"foundation","pareto_weight":9,"prerequisites":[],"hour":1},
           {"id":"errors","name":"Error handling","why_it_matters":"real failures","pareto_weight":7,"prerequisites":["basics"],"hour":2}],
 "hours":[{"hour":1,"title":"First widget","objectives":["make a widget"],"skill_ids":["basics"],"capstone_slice":"skeleton"},
          {"hour":2,"title":"Broken widgets","objectives":["fix a widget"],"skill_ids":["errors"],"capstone_slice":"errors"}],
 "capstone":{"title":"Widget service","description":"a service","acceptance_criteria":["runs"]},
 "diagnostic":[{"id":"d1","question":"What is a widget?","skill_id":"basics","answer_key":"a thing"},
               {"id":"d2","question":"How do widgets fail?","skill_id":"errors","answer_key":"loudly"}],
 "research":{"stackexchange_site":"","tags":[],"queries":[]},
 "lab_constraints":"native only"}
JSON
    ;;
  calibrate)
    cat > "$out" <<'JSON'
{"diagnostic_results":[{"id":"d1","score":1,"feedback":"right"},{"id":"d2","score":0,"feedback":"they fail loudly"}],
 "skill_mastery":[{"skill_id":"basics","mastery":0.5},{"skill_id":"errors","mastery":0.0}],
 "level_estimate":"advanced beginner","plan_changes":"Compressed basics.",
 "hours":[{"hour":1,"title":"First widget (compressed)","objectives":["make a widget"],"skill_ids":["basics"],"capstone_slice":"skeleton"},
          {"hour":2,"title":"Broken widgets","objectives":["fix a widget"],"skill_ids":["errors"],"capstone_slice":"errors"}],
 "issues":[{"id":"i1","title":"Widget explodes","symptom":"boom","root_cause":"null","fix":"check null","skill_id":"errors","source_url":"https://stackoverflow.com/q/1"}],
 "misconceptions":["widgets never fail"]}
JSON
    ;;
  hour-*)
    n="${label#hour-}"
    mkdir -p "$cwd/lessons" "$cwd/lab/hour-$n"
    printf '# Hour %s lesson\n\nWidgets are things.\n' "$n" > "$cwd/lessons/hour-$n.md"
    printf '#!/bin/sh\necho "PASS t1: widget exists"\necho "FAIL t2: widget broken"\nexit 1\n' > "$cwd/lab/hour-$n/check.sh"
    cat > "$out" <<JSON
{"lesson_file":"lessons/hour-$n.md","predict_question":"What makes a widget?",
 "tasks":[{"id":"t1","title":"Complete the widget","kind":"worked_completion","instructions":"Fill the TODO","success_criteria":["test passes"],"hints":["look at TODO","use make()","make(widget)"],"minutes":10},
          {"id":"t2","title":"Fix the widget","kind":"break_fix","instructions":"It explodes","success_criteria":["no boom"],"hints":["read the log"],"minutes":10}],
 "setup":{"script":"","teardown_script":"","est_ram_mb":50,"est_disk_mb":5,"notes":""},
 "check_script":"lab/hour-$n/check.sh",
 "cards":[{"skill_id":"basics","front":"What is a widget?","back":"a thing"},{"skill_id":"errors","front":"How do widgets fail?","back":"loudly"}],
 "exit_quiz":[{"id":"q1","question":"Define widget","skill_id":"basics","kind":"recall","answer_key":"a thing"},
              {"id":"q2","question":"Fix a boom","skill_id":"errors","kind":"apply","answer_key":"check null"}],
 "feynman_prompt":"Explain widgets.","used_issue_ids":["i1"]}
JSON
    ;;
  check)
    echo '{"task_results":[{"task_id":"t1","passed":true,"feedback":"good"},{"task_id":"t2","passed":false,"feedback":"null check missing"}],"misconceptions":["null is fine"],"next_step":"add the null check"}' > "$out"
    ;;
  assess)
    echo '{"results":[{"id":"q1","score":1,"feedback":"yes"},{"id":"q2","score":0.25,"feedback":"check null first"}],"feynman":{"score":0.8,"feedback":"clear","misconceptions":[]},"skill_mastery":[{"skill_id":"basics","mastery":0.9},{"skill_id":"errors","mastery":0.3}],"new_cards":[{"skill_id":"errors","front":"First step when a widget explodes?","back":"check for null"}],"coach_note":"Reinforce error handling."}' > "$out"
    ;;
  ask)
    echo "A widget is a thing." > "$out"
    ;;
  *)
    echo "fake codex: unknown label $label" >&2
    exit 3
    ;;
esac
echo '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1,"reasoning_output_tokens":0}}'
