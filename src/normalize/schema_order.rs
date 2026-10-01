//! Which schema violation an `ACV010` error reports.
//!
//! `acc` names the instance location of one violation, and that location is part of its output,
//! so it must not depend on the evaluation order of the `jsonschema` release in use. Since 0.30
//! `jsonschema` runs a schema object's keywords cheapest-first (`required` before `properties`,
//! for example), while `acc` has always reported the first violation in the order 0.29 visited
//! them: depth first, the keywords of each schema object in lexicographic order, with
//! `properties` folded into a sibling `additionalProperties` (members in key order, then the
//! undeclared-member violation), `then`/`else` in the place of `if`, and array items and map
//! members in instance order.
//!
//! [`key`] turns a violation's evaluation path and instance path into a sort key for that order,
//! so the reported violation is the smallest key among all of them. The walk covers the keywords
//! the embedded schemas use.

use serde_json::Value;

/// One step of the walk. Steps at the same position are always of the same kind for two
/// violations under the same schema object, so the derived order is only compared within kinds.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Step {
    /// A keyword of the current schema object, by name.
    Keyword(String),
    /// Within a folded `additionalProperties`: 0 for a member's own violations, 1 for the
    /// violation that lists undeclared members, which 0.29 reported after them.
    Member(u8),
    /// An object member, by name.
    Key(String),
    /// An array item, or a subschema of `allOf`, by position.
    Index(u64),
}

fn segments(pointer: &str) -> Vec<String> {
    pointer
        .split('/')
        .skip(1)
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect()
}

/// The sort key of a violation at `evaluation_path` (keywords from the root of `schema`, `$ref`
/// included) and `instance_path`, both JSON Pointers.
pub(super) fn key(schema: &Value, evaluation_path: &str, instance_path: &str) -> Vec<Step> {
    let eval = segments(evaluation_path);
    let mut instance = segments(instance_path).into_iter();
    let mut member = || instance.next().unwrap_or_default();
    let mut out = Vec::new();
    let mut node = schema;
    let mut i = 0;
    while let (Some(keyword), Some(object)) = (eval.get(i), node.as_object()) {
        let keyword = keyword.as_str();
        let next = match keyword {
            "$ref" => {
                out.push(Step::Keyword(keyword.into()));
                i += 1;
                object["$ref"]
                    .as_str()
                    .and_then(|r| r.strip_prefix('#'))
                    .and_then(|pointer| schema.pointer(pointer))
            }
            "properties" => {
                if object.contains_key("additionalProperties") {
                    out.push(Step::Keyword("additionalProperties".into()));
                    out.push(Step::Member(0));
                } else {
                    out.push(Step::Keyword(keyword.into()));
                }
                out.push(Step::Key(member()));
                let name = eval.get(i + 1);
                i += 2;
                name.and_then(|name| object[keyword].get(name))
            }
            "additionalProperties" => {
                out.push(Step::Keyword(keyword.into()));
                i += 1;
                if i == eval.len() {
                    out.push(Step::Member(1));
                    None
                } else {
                    out.push(Step::Member(0));
                    out.push(Step::Key(member()));
                    Some(&object[keyword])
                }
            }
            "items" => {
                out.push(Step::Keyword(keyword.into()));
                i += 1;
                if i == eval.len() {
                    None
                } else {
                    out.push(Step::Index(member().parse().unwrap_or(u64::MAX)));
                    Some(&object[keyword])
                }
            }
            "allOf" => {
                out.push(Step::Keyword(keyword.into()));
                let index = eval.get(i + 1).and_then(|s| s.parse::<usize>().ok());
                i += 2;
                index.and_then(|index| {
                    out.push(Step::Index(index as u64));
                    object[keyword].get(index)
                })
            }
            "then" | "else" => {
                out.push(Step::Keyword("if".into()));
                i += 1;
                Some(&object[keyword])
            }
            _ => {
                out.push(Step::Keyword(keyword.into()));
                None
            }
        };
        match next {
            Some(n) => node = n,
            None => break,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn first(schema: &Value, instance: &Value) -> Option<String> {
        let validator = jsonschema::options()
            .should_validate_formats(true)
            .build(schema)
            .unwrap();
        validator
            .iter_errors(instance)
            .map(|e| {
                let path = e.instance_path().to_string();
                (key(schema, &e.evaluation_path().to_string(), &path), path)
            })
            .reduce(|best, e| if e.0 < best.0 { e } else { best })
            .map(|(_, path)| path)
    }

    #[test]
    fn member_violations_come_before_required() {
        let schema = json!({
            "type": "object",
            "required": ["a", "z"],
            "properties": {"a": {"type": "string"}, "z": {"type": "string"}},
            "additionalProperties": false
        });
        assert_eq!(first(&schema, &json!({"a": 1})).as_deref(), Some("/a"));
        assert_eq!(first(&schema, &json!({"a": "x"})).as_deref(), Some(""));
    }

    #[test]
    fn member_violations_come_before_undeclared_members() {
        let schema = json!({
            "type": "object",
            "properties": {"z": {"type": "string"}},
            "additionalProperties": false
        });
        assert_eq!(
            first(&schema, &json!({"a": 1, "z": 1})).as_deref(),
            Some("/z")
        );
    }

    #[test]
    fn items_are_visited_in_order_and_depth_first() {
        let schema = json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {"a": {"type": "string"}, "b": {"type": "string"}}
            }
        });
        let instance = json!([{"a": "x", "b": 1}, {"a": 1, "b": "x"}]);
        assert_eq!(first(&schema, &instance).as_deref(), Some("/0/b"));
    }
}
