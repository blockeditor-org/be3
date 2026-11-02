use super::*;

#[derive(Clone, Debug, Default, serde::Deserialize, PartialEq, serde::Serialize)]
struct Strict {
    width: f32,
}

#[derive(Clone, Debug, Default, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(default)]
struct Lenient {
    width: f32,
    inner: Strict,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Shape {
    lenient: Lenient,
    strict: Strict,
}

#[test]
fn a_register_that_cannot_read_without_one_of_its_fields_is_reported() {
    let problems = Shape::kind().registers_accept_missing_fields();
    assert_eq!(problems.len(), 2);
    assert!(problems[0].starts_with("shape.lenient does not read without inner.width"));
    assert!(problems[1].starts_with("shape.strict does not read without width"));
}
