//! JSON Schemas for Codex structured output (`codex exec --output-schema`).
//! Strict mode: every object lists all properties as required and forbids
//! extras, so Guru can rely on the shape it gets back.

use serde_json::json;
use serde_json::Map;
use serde_json::Value;

use crate::model::Arena;

fn s() -> Value {
    json!({"type": "string"})
}
fn int() -> Value {
    json!({"type": "integer"})
}
fn num() -> Value {
    json!({"type": "number"})
}
fn boolean() -> Value {
    json!({"type": "boolean"})
}
fn arr(items: Value) -> Value {
    json!({"type": "array", "items": items})
}
fn one_of(values: &[&str]) -> Value {
    json!({"type": "string", "enum": values})
}
fn obj(props: &[(&str, Value)]) -> Value {
    let mut properties = Map::new();
    for (k, v) in props {
        properties.insert((*k).to_string(), v.clone());
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": props.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
        "additionalProperties": false,
    })
}

fn card() -> Value {
    obj(&[("skill_id", s()), ("front", s()), ("back", s())])
}

fn issue() -> Value {
    obj(&[
        ("id", s()),
        ("title", s()),
        ("symptom", s()),
        ("root_cause", s()),
        ("fix", s()),
        ("skill_id", s()),
        ("source_url", s()),
    ])
}

fn hour() -> Value {
    obj(&[
        ("hour", int()),
        ("title", s()),
        ("objectives", arr(s())),
        ("skill_ids", arr(s())),
        ("capstone_slice", s()),
    ])
}

pub fn plan() -> Value {
    obj(&[
        ("title", s()),
        (
            "domain",
            one_of(&[
                "software",
                "devops_infra",
                "data_ml",
                "spoken_language",
                "academic_concept",
                "creative_physical",
                "professional_soft_skill",
                "other",
            ]),
        ),
        ("arena", one_of(&Arena::ALL)),
        ("target_performance", s()),
        ("out_of_scope", arr(s())),
        ("level_estimate", s()),
        (
            "skills",
            arr(obj(&[
                ("id", s()),
                ("name", s()),
                ("why_it_matters", s()),
                ("pareto_weight", int()),
                ("prerequisites", arr(s())),
                ("hour", int()),
            ])),
        ),
        ("hours", arr(hour())),
        (
            "capstone",
            obj(&[
                ("title", s()),
                ("description", s()),
                ("acceptance_criteria", arr(s())),
            ]),
        ),
        (
            "diagnostic",
            arr(obj(&[
                ("id", s()),
                ("question", s()),
                ("skill_id", s()),
                ("answer_key", s()),
            ])),
        ),
        (
            "research",
            obj(&[
                ("stackexchange_site", s()),
                ("tags", arr(s())),
                ("queries", arr(s())),
            ]),
        ),
        ("lab_constraints", s()),
    ])
}

pub fn calibrate() -> Value {
    obj(&[
        (
            "diagnostic_results",
            arr(obj(&[("id", s()), ("score", num()), ("feedback", s())])),
        ),
        (
            "skill_mastery",
            arr(obj(&[("skill_id", s()), ("mastery", num())])),
        ),
        ("level_estimate", s()),
        ("plan_changes", s()),
        ("hours", arr(hour())),
        ("issues", arr(issue())),
        ("misconceptions", arr(s())),
    ])
}

pub fn hour_pack() -> Value {
    obj(&[
        ("lesson_file", s()),
        ("predict_question", s()),
        (
            "tasks",
            arr(obj(&[
                ("id", s()),
                ("title", s()),
                (
                    "kind",
                    one_of(&[
                        "worked_completion",
                        "independent",
                        "break_fix",
                        "capstone",
                        "roleplay",
                        "drill",
                        "problem",
                    ]),
                ),
                ("instructions", s()),
                ("success_criteria", arr(s())),
                ("hints", arr(s())),
                ("minutes", int()),
            ])),
        ),
        (
            "setup",
            obj(&[
                ("script", s()),
                ("teardown_script", s()),
                ("est_ram_mb", int()),
                ("est_disk_mb", int()),
                ("notes", s()),
            ]),
        ),
        ("check_script", s()),
        ("cards", arr(card())),
        (
            "exit_quiz",
            arr(obj(&[
                ("id", s()),
                ("question", s()),
                ("skill_id", s()),
                (
                    "kind",
                    one_of(&["recall", "apply", "interleaved", "diagnose"]),
                ),
                ("answer_key", s()),
            ])),
        ),
        ("feynman_prompt", s()),
        ("used_issue_ids", arr(s())),
    ])
}

pub fn lab_check() -> Value {
    obj(&[
        (
            "task_results",
            arr(obj(&[
                ("task_id", s()),
                ("passed", boolean()),
                ("feedback", s()),
            ])),
        ),
        ("misconceptions", arr(s())),
        ("next_step", s()),
    ])
}

pub fn assess() -> Value {
    obj(&[
        (
            "results",
            arr(obj(&[("id", s()), ("score", num()), ("feedback", s())])),
        ),
        (
            "feynman",
            obj(&[
                ("score", num()),
                ("feedback", s()),
                ("misconceptions", arr(s())),
            ]),
        ),
        (
            "skill_mastery",
            arr(obj(&[("skill_id", s()), ("mastery", num())])),
        ),
        ("new_cards", arr(card())),
        ("coach_note", s()),
    ])
}

pub fn hint() -> Value {
    obj(&[("hint", s()), ("question_back", s())])
}

pub fn incident() -> Value {
    obj(&[
        ("title", s()),
        ("directory", s()),
        ("briefing", s()),
        ("hints", arr(s())),
        ("issue_id", s()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Walk a schema and assert every object is strict.
    fn assert_strict(v: &Value, path: &str) {
        if v.get("type") == Some(&json!("object")) {
            let props = v["properties"].as_object().expect("properties");
            let required: Vec<&str> = v["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_str().unwrap())
                .collect();
            assert_eq!(v["additionalProperties"], json!(false), "{path}");
            for k in props.keys() {
                assert!(required.contains(&k.as_str()), "{path}.{k} not required");
            }
            for (k, p) in props {
                assert_strict(p, &format!("{path}.{k}"));
            }
        }
        if let Some(items) = v.get("items") {
            assert_strict(items, &format!("{path}[]"));
        }
    }

    #[test]
    fn all_schemas_are_strict() {
        for (name, schema) in [
            ("plan", plan()),
            ("calibrate", calibrate()),
            ("hour_pack", hour_pack()),
            ("lab_check", lab_check()),
            ("assess", assess()),
            ("hint", hint()),
            ("incident", incident()),
        ] {
            assert_strict(&schema, name);
        }
    }
}
