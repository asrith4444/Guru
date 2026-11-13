//! JSON Schemas for Codex structured output (`codex exec --output-schema`).
//! Strict style: every object lists all of its properties as required and
//! forbids extras. Widgets use `anyOf`, one shape per widget type.

use serde_json::json;
use serde_json::Map;
use serde_json::Value;

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
pub(crate) fn obj(props: &[(&str, Value)]) -> Value {
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

const NODE_STATUS: &[&str] = &["healthy", "degraded", "down", "compromised", "unknown"];

pub fn scope() -> Value {
    let level = obj(&[("hours", num()), ("can_do", s())]);
    obj(&[
        ("title", s()),
        ("breadth", int()),
        ("beginner", level.clone()),
        ("intermediate", level.clone()),
        ("advanced", level),
        ("simulated_tools", arr(s())),
        ("note", s()),
    ])
}

fn sim_plan() -> Value {
    obj(&[
        ("id", s()),
        ("title", s()),
        ("kind", one_of(&["incident", "tool", "guided", "roleplay"])),
        ("tool", s()),
        ("premise", s()),
    ])
}

pub fn plan() -> Value {
    obj(&[
        ("title", s()),
        ("summary", s()),
        ("target_performance", s()),
        ("out_of_scope", arr(s())),
        (
            "skills",
            arr(obj(&[
                ("id", s()),
                ("name", s()),
                ("why", s()),
                ("weight", int()),
            ])),
        ),
        (
            "modules",
            arr(obj(&[
                ("id", s()),
                ("title", s()),
                ("minutes", int()),
                ("summary", s()),
                ("objectives", arr(s())),
                ("skill_ids", arr(s())),
                ("sims", arr(sim_plan())),
            ])),
        ),
        ("capstone", sim_plan()),
        (
            "research",
            obj(&[
                ("stackexchange_site", s()),
                ("tags", arr(s())),
                ("queries", arr(s())),
            ]),
        ),
    ])
}

pub fn module() -> Value {
    obj(&[
        ("lesson", s()),
        ("key_points", arr(s())),
        (
            "cards",
            arr(obj(&[("skill_id", s()), ("front", s()), ("back", s())])),
        ),
        (
            "quiz",
            arr(obj(&[
                ("id", s()),
                ("kind", one_of(&["single", "multi", "text"])),
                ("prompt", s()),
                ("options", arr(s())),
                ("correct", arr(int())),
                ("accept", arr(s())),
                ("explanation", s()),
                ("skill_id", s()),
            ])),
        ),
    ])
}

fn effects() -> Value {
    obj(&[
        ("set_flags", arr(s())),
        (
            "node_status",
            arr(obj(&[("node_id", s()), ("status", one_of(NODE_STATUS))])),
        ),
        ("message", s()),
    ])
}

fn series() -> Value {
    obj(&[("name", s()), ("points", arr(num()))])
}

fn widget(kind: &str, props: &[(&str, Value)]) -> Value {
    let mut all = vec![("type", one_of(&[kind])), ("id", s()), ("title", s())];
    all.extend(props.iter().cloned());
    obj(&all)
}

fn widgets() -> Value {
    json!({"anyOf": [
        widget("markdown", &[("text", s())]),
        widget("stat", &[("value", s()), ("unit", s()), ("status", one_of(&["ok", "warn", "crit"]))]),
        widget("timeseries", &[("unit", s()), ("labels", arr(s())), ("series", arr(series())), ("thresholds", arr(num()))]),
        widget("table", &[("columns", arr(s())), ("rows", arr(arr(s())))]),
        widget("logs", &[("stream_id", s())]),
        widget("query", &[
            ("language", s()),
            ("placeholder", s()),
            ("responses", arr(obj(&[
                ("pattern", s()),
                ("result", one_of(&["table", "timeseries", "text", "error"])),
                ("text", s()),
                ("columns", arr(s())),
                ("rows", arr(arr(s()))),
                ("labels", arr(s())),
                ("series", arr(series())),
                ("effects", effects()),
            ]))),
            ("fallback", s()),
        ]),
        widget("form", &[
            ("fields", arr(obj(&[
                ("id", s()),
                ("label", s()),
                ("kind", one_of(&["text", "number", "select", "toggle", "textarea"])),
                ("options", arr(s())),
                ("value", s()),
            ]))),
            ("submit_label", s()),
            ("rules", arr(obj(&[
                ("conditions", arr(obj(&[("field", s()), ("pattern", s())]))),
                ("ok", boolean()),
                ("message", s()),
                ("effects", effects()),
            ]))),
            ("fallback", s()),
        ]),
        widget("actions", &[("buttons", arr(obj(&[("label", s()), ("message", s()), ("effects", effects())])))]),
        widget("editor", &[
            ("language", s()),
            ("path", s()),
            ("text", s()),
            ("checks", arr(obj(&[("must_match", boolean()), ("pattern", s()), ("message", s())]))),
            ("effects", effects()),
        ]),
        widget("chat", &[
            ("persona", s()),
            ("text", s()),
            ("turns", arr(obj(&[("expect", s()), ("reply", s()), ("feedback", s()), ("effects", effects())]))),
            ("fallback", s()),
        ]),
    ]})
}

pub fn simulation() -> Value {
    obj(&[
        ("id", s()),
        ("title", s()),
        ("kind", one_of(&["incident", "tool", "guided", "roleplay"])),
        ("tool", s()),
        ("minutes", int()),
        ("skill_ids", arr(s())),
        ("briefing", s()),
        (
            "world",
            obj(&[
                (
                    "nodes",
                    arr(obj(&[
                        ("id", s()),
                        ("label", s()),
                        (
                            "kind",
                            one_of(&[
                                "server",
                                "database",
                                "load_balancer",
                                "container",
                                "service",
                                "client",
                                "cloud",
                                "queue",
                                "firewall",
                                "app",
                                "person",
                            ]),
                        ),
                        ("status", one_of(NODE_STATUS)),
                        ("detail", s()),
                        ("x", num()),
                        ("y", num()),
                    ])),
                ),
                (
                    "links",
                    arr(obj(&[("from", s()), ("to", s()), ("label", s())])),
                ),
            ]),
        ),
        (
            "hosts",
            arr(obj(&[
                ("node_id", s()),
                ("hostname", s()),
                ("user", s()),
                ("cwd", s()),
                ("motd", s()),
                ("files", arr(obj(&[("path", s()), ("content", s())]))),
                (
                    "processes",
                    arr(obj(&[
                        ("pid", int()),
                        ("user", s()),
                        ("cpu", num()),
                        ("mem", num()),
                        ("command", s()),
                    ])),
                ),
                (
                    "services",
                    arr(obj(&[
                        ("name", s()),
                        ("status", one_of(&["active", "inactive", "failed"])),
                        ("detail", s()),
                    ])),
                ),
                (
                    "commands",
                    arr(obj(&[
                        ("pattern", s()),
                        ("output", s()),
                        ("effects", effects()),
                    ])),
                ),
            ])),
        ),
        (
            "logs",
            arr(obj(&[
                ("id", s()),
                ("name", s()),
                ("node_id", s()),
                (
                    "lines",
                    arr(obj(&[
                        ("ts", s()),
                        (
                            "level",
                            one_of(&["debug", "info", "warn", "error", "critical"]),
                        ),
                        ("message", s()),
                    ])),
                ),
            ])),
        ),
        (
            "apps",
            arr(obj(&[
                ("id", s()),
                ("name", s()),
                ("node_id", s()),
                ("accent", s()),
                (
                    "screens",
                    arr(obj(&[
                        ("id", s()),
                        ("title", s()),
                        ("widgets", arr(widgets())),
                    ])),
                ),
            ])),
        ),
        (
            "objectives",
            arr(obj(&[
                ("id", s()),
                ("text", s()),
                ("kind", one_of(&["answer", "state"])),
                ("question", s()),
                ("options", arr(s())),
                ("correct", arr(int())),
                ("accept", arr(s())),
                ("flags", arr(s())),
                ("explanation", s()),
            ])),
        ),
        ("hints", arr(s())),
        ("debrief", s()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_strict(v: &Value, path: &str) {
        if let Some(any) = v.get("anyOf").and_then(Value::as_array) {
            for (i, alt) in any.iter().enumerate() {
                assert_strict(alt, &format!("{path}|{i}"));
            }
            return;
        }
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
            ("scope", scope()),
            ("plan", plan()),
            ("module", module()),
            ("simulation", simulation()),
        ] {
            assert_strict(&schema, name);
        }
    }

    #[test]
    fn demo_simulations_conform_to_the_schema_shape() {
        // Every field the schema requires must exist in the demo sims, so the
        // hand-written demos exercise the same shape Codex produces.
        fn check(v: &Value, schema: &Value, path: &str) {
            if let Some(any) = schema.get("anyOf").and_then(Value::as_array) {
                let ty = v.get("type").and_then(Value::as_str).unwrap_or("");
                let alt = any
                    .iter()
                    .find(|a| a["properties"]["type"]["enum"][0] == json!(ty))
                    .unwrap_or_else(|| panic!("{path}: no widget shape for '{ty}'"));
                return check(v, alt, path);
            }
            match schema.get("type").and_then(Value::as_str) {
                Some("object") => {
                    let o = v
                        .as_object()
                        .unwrap_or_else(|| panic!("{path}: expected object"));
                    for k in schema["required"].as_array().unwrap() {
                        let k = k.as_str().unwrap();
                        let child = o.get(k).unwrap_or_else(|| panic!("{path}.{k} missing"));
                        check(child, &schema["properties"][k], &format!("{path}.{k}"));
                    }
                }
                Some("array") => {
                    for (i, item) in v
                        .as_array()
                        .unwrap_or_else(|| panic!("{path}: expected array"))
                        .iter()
                        .enumerate()
                    {
                        check(item, &schema["items"], &format!("{path}[{i}]"));
                    }
                }
                Some("string") => {
                    assert!(v.is_string(), "{path}: expected string");
                    if let Some(e) = schema.get("enum").and_then(Value::as_array) {
                        assert!(e.contains(v), "{path}: {v} not in {e:?}");
                    }
                }
                Some("integer") => assert!(v.is_u64() || v.is_i64(), "{path}: expected integer"),
                Some("number") => assert!(v.is_number(), "{path}: expected number"),
                Some("boolean") => assert!(v.is_boolean(), "{path}: expected boolean"),
                _ => {}
            }
        }
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../ui/demo/sims");
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            let v: Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
            check(&v, &simulation(), &p.display().to_string());
        }
    }
}
