//! A plain aligned-column terminal summary.

use crate::model::{Confidence, Manifest, Operator, Severity, Status};
use crate::policy::RULES;

/// Strip terminal control characters from untrusted identifiers.
fn plain(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

/// Rows with two spaces between columns; every column but the last is padded to its widest cell.
fn columns(rows: &[Vec<String>]) -> String {
    let ncol = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0; ncol];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    for row in rows {
        let last = row.len().saturating_sub(1);
        for (i, cell) in row.iter().enumerate() {
            out.push_str(cell);
            if i < last {
                let pad = widths[i] - cell.chars().count() + 2;
                out.extend(std::iter::repeat_n(' ', pad));
            }
        }
        out.push('\n');
    }
    out
}

pub fn render(m: &Manifest) -> String {
    let mut rows = vec![
        ["CHANGE", "AUTHOR", "OPERATOR", "RESULT"]
            .map(String::from)
            .to_vec(),
    ];
    for c in &m.events.changes {
        let findings = m.findings.iter().filter(|f| f.change_id == c.id);
        let result = if findings.clone().any(|f| f.severity >= Severity::Medium) {
            "FAIL"
        } else if findings.clone().next().is_some() {
            "WARN"
        } else if !m.events.window.complete
            || !c.reviews_complete
            || m.assessments
                .iter()
                .any(|a| a.change_id == c.id && a.status == Status::Unknown)
        {
            "UNKNOWN"
        } else {
            "PASS"
        };
        rows.push(vec![
            plain(&c.id),
            plain(&c.author.actor_id),
            match c.agent_operator.as_ref() {
                Some(Operator {
                    actor_id: Some(id),
                    confidence: Confidence::Derived,
                    ..
                }) => format!("{} (derived)", plain(id)),
                Some(Operator {
                    actor_id: Some(id), ..
                }) => plain(id),
                _ => "-".into(),
            },
            result.into(),
        ]);
    }
    let mut out = columns(&rows);
    let s = &m.summary;
    out.push('\n');
    for (count, label) in [
        (s.changes, "changes evaluated"),
        (s.human_authored, "human-authored"),
        (s.agent_authored, "agent-authored"),
        (s.agent_operator_unknown, "agent operator unknown"),
        (s.clean, "clean"),
        (s.with_findings, "with findings"),
        (s.indeterminate, "indeterminate"),
    ] {
        out.push_str(&format!("{count} {label}\n"));
    }
    let counts: Vec<Vec<String>> = RULES
        .iter()
        .map(|rule| {
            vec![
                rule.code().into(),
                rule.name().to_string(),
                m.findings
                    .iter()
                    .filter(|f| f.rule_id == *rule)
                    .count()
                    .to_string(),
            ]
        })
        .collect();
    out.push_str(&columns(&counts));
    if !m.events.window.complete {
        out.push_str(&format!(
            "INCOMPLETE: {}\n",
            plain(m.events.window.reason.as_deref().unwrap_or("unspecified"))
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_pad_all_but_the_last() {
        let rows = vec![
            vec!["a".to_string(), "bb".to_string(), "c".to_string()],
            vec!["aaa".to_string(), "b".to_string(), "cc".to_string()],
        ];
        assert_eq!(columns(&rows), "a    bb  c\naaa  b   cc\n");
    }

    #[test]
    fn control_characters_are_removed() {
        assert_eq!(plain("a\x1b[31mb\n"), "a[31mb");
    }
}
