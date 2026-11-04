use super::*;
use crate::schema::{check_frozen, freeze};

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "board")]
struct Grown {
    title: String,
    votes: Count,
    columns: List<Column>,
    archived: bool,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "board")]
struct Shrunk {
    title: String,
    columns: List<Column>,
}

fn part<'a>(source: &'a str, name: &str, open: &str, close: &str) -> &'a str {
    let start = source.find(name).expect("the constant is there");
    let rest = &source[start..];
    let open = rest.find(open).expect("a string") + open.len();
    let close = rest[open..].find(close).expect("a closed string");
    &rest[open..open + close]
}

#[test]
fn a_frozen_document_keeps_loading_into_a_schema_that_only_adds() {
    let source = freeze(&board());
    let registry = part(&source, "REGISTRY", "r#\"", "\"#;");
    let document = part(&source, "DOCUMENT", "\"", "\";");

    assert_eq!(check_frozen::<Board>(registry, document), Ok(()));
    assert_eq!(check_frozen::<Grown>(registry, document), Ok(()));
    let problems = check_frozen::<Shrunk>(registry, document).expect_err("votes is gone");
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("board.votes"));
}
