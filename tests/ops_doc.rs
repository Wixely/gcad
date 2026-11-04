use std::collections::BTreeMap;

const CATEGORIES: [&str; 15] = [
    "sketch",
    "workplane",
    "features",
    "holes",
    "blends",
    "bodies",
    "assembly",
    "output",
    "inspect",
    "simulation",
    "progress",
    "stored",
    "mesh",
    "inspect_view",
    "calc",
];

fn root() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
}

fn tests_in(category: &str) -> BTreeMap<String, Option<String>> {
    let source =
        std::fs::read_to_string(format!("{}/tests/{category}.rs", root())).expect("category file");
    let lines: Vec<&str> = source.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == "#[test]")
        .filter_map(|(i, _)| {
            let rest = &lines[i + 1..];
            let at = rest.iter().position(|l| l.trim().starts_with("fn "))?;
            let (attributes, signature) = (&rest[..at], rest[at]);
            let name = signature
                .trim()
                .strip_prefix("fn ")?
                .split('(')
                .next()?
                .to_string();
            let ignore = attributes.iter().find_map(|a| {
                a.trim()
                    .strip_prefix("#[ignore = \"")?
                    .strip_suffix("\"]")
                    .map(str::to_string)
            });
            Some((format!("{category}::{name}"), ignore))
        })
        .collect()
}

struct Row {
    operation: String,
    status: String,
    tests: Vec<String>,
}

fn rows() -> Vec<Row> {
    let doc = std::fs::read_to_string(format!("{}/docs/ops.md", root())).expect("docs/ops.md");
    doc.lines()
        .filter(|line| {
            line.starts_with('|') && !line.starts_with("|---") && !line.starts_with("| operation")
        })
        .map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split(" | ").map(str::trim).collect();
            let tests = cells
                .last()
                .expect("a tests column")
                .split('`')
                .skip(1)
                .step_by(2)
                .map(str::to_string)
                .collect();
            Row {
                operation: cells[0].to_string(),
                status: cells[1].to_string(),
                tests,
            }
        })
        .collect()
}

#[test]
fn every_operation_has_a_test_that_matches_its_status() {
    let tests: BTreeMap<String, Option<String>> =
        CATEGORIES.iter().flat_map(|c| tests_in(c)).collect();
    let rows = rows();
    let mut problems = Vec::new();
    for row in &rows {
        if row.tests.is_empty() {
            problems.push(format!("`{}` lists no tests", row.operation));
        }
        if row.status != "supported" && row.status != "missing" {
            problems.push(format!(
                "`{}` has status `{}`, use supported or missing",
                row.operation, row.status
            ));
        }
        for test in &row.tests {
            match (tests.get(test), row.status.as_str()) {
                (None, _) => problems.push(format!(
                    "`{}` lists `{test}`, which does not exist",
                    row.operation
                )),
                (Some(Some(reason)), "supported") if reason.starts_with("missing") => problems
                    .push(format!(
                        "`{}` is supported but `{test}` is ignored as {reason}",
                        row.operation
                    )),
                (Some(ignore), "missing")
                    if !ignore.as_deref().is_some_and(|r| r.starts_with("missing")) =>
                {
                    problems.push(format!(
                        "`{}` is missing but `{test}` is not ignored as missing",
                        row.operation
                    ))
                }
                _ => {}
            }
        }
    }
    let listed: Vec<&String> = rows.iter().flat_map(|row| &row.tests).collect();
    for test in tests.keys().filter(|t| !listed.contains(t)) {
        problems.push(format!("`{test}` is not listed in docs/ops.md"));
    }
    assert!(
        problems.is_empty(),
        "docs/ops.md and the tests disagree:\n{}",
        problems.join("\n")
    );
}
