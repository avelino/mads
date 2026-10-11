//! The one rule that turns what the feed wants and what the account has into a daily step.
//! `TABLE` owns it. The Feed formula and the README table are both generated from it.

use super::columns::FeedColumn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NextStep {
    Create,
    Enable,
    Pause,
}

impl NextStep {
    pub fn label(self) -> &'static str {
        match self {
            NextStep::Create => "CREATE",
            NextStep::Enable => "ENABLE",
            NextStep::Pause => "PAUSE",
        }
    }
}

/// `(action, current_status, next_step)`. An empty status means the ad group is not in the
/// account. Pairs not listed here give no step.
pub(crate) const TABLE: [(&str, &str, Option<NextStep>); 6] = [
    ("Add", "", Some(NextStep::Create)),
    ("Add", "Paused", Some(NextStep::Enable)),
    ("Add", "Enabled", None),
    ("-", "Enabled", Some(NextStep::Pause)),
    ("-", "Paused", None),
    ("-", "", None),
];

/// The `next_step` formula of Feed row `row` (Excel row number). Empty on rows whose
/// `automated` cell is not `Yes`. The pairs of `TABLE` are disjoint, so rows without a step
/// fall through to the final `""`.
pub(crate) fn formula(row: usize) -> String {
    let action = format!("${}{row}", FeedColumn::Action.letter());
    let status = format!("${}{row}", FeedColumn::CurrentStatus.letter());
    let automated = format!("${}{row}", FeedColumn::Automated.letter());
    let steps: Vec<(&str, &str, NextStep)> = TABLE
        .iter()
        .filter_map(|&(a, s, step)| step.map(|step| (a, s, step)))
        .collect();
    let mut chain = String::from("\"\"");
    for (a, s, step) in steps.iter().rev() {
        chain = format!(
            "IF(AND({action}=\"{a}\",{status}=\"{s}\"),\"{}\",{chain})",
            step.label()
        );
    }
    format!("=IF({automated}<>\"Yes\",\"\",{chain})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formula_renders_the_table_as_nested_ifs() {
        assert_eq!(
            formula(5),
            "=IF($C5<>\"Yes\",\"\",\
             IF(AND($A5=\"Add\",$AK5=\"\"),\"CREATE\",\
             IF(AND($A5=\"Add\",$AK5=\"Paused\"),\"ENABLE\",\
             IF(AND($A5=\"-\",$AK5=\"Enabled\"),\"PAUSE\",\"\"))))"
        );
    }

    #[test]
    fn every_step_of_the_table_is_in_the_formula() {
        let f = formula(12);
        for (action, status, step) in TABLE {
            let clause = format!("AND($A12=\"{action}\",$AK12=\"{status}\")");
            match step {
                Some(step) => {
                    assert!(
                        f.contains(&format!("{clause},\"{}\"", step.label())),
                        "{clause}"
                    )
                }
                None => assert!(!f.contains(&clause), "{clause} has no step"),
            }
        }
    }

    #[test]
    fn labels() {
        assert_eq!(NextStep::Create.label(), "CREATE");
        assert_eq!(NextStep::Enable.label(), "ENABLE");
        assert_eq!(NextStep::Pause.label(), "PAUSE");
    }
}
