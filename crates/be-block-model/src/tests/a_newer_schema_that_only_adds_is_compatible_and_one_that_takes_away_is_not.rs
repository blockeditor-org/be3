use super::*;
use crate::schema::compatible;

#[derive(Clone, Debug, Default, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(default)]
struct Style {
    width: f32,
}

#[derive(Clone, Debug, Default, serde::Deserialize, PartialEq, serde::Serialize)]
#[serde(default)]
struct WiderStyle {
    width: f32,
    dashed: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
struct ThickStyle {
    width: f32,
}

impl Default for ThickStyle {
    fn default() -> Self {
        Self { width: 4.0 }
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "shape")]
struct Frozen {
    name: String,
    style: Style,
    parts: List<Card>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "shape")]
struct Added {
    #[model(alias = "name")]
    title: String,
    style: WiderStyle,
    parts: List<Card>,
    count: Count,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "shape")]
struct Removed {
    style: Style,
    parts: List<Card>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "shape")]
struct Retyped {
    name: Count,
    style: ThickStyle,
    parts: List<Card>,
}

#[test]
fn a_newer_schema_that_only_adds_is_compatible_and_one_that_takes_away_is_not() {
    let frozen = Frozen::kind().describe();
    assert_eq!(compatible(&frozen, &frozen), Ok(()));
    assert_eq!(compatible(&frozen, &Added::kind().describe()), Ok(()));

    let removed = compatible(&frozen, &Removed::kind().describe()).expect_err("a field is gone");
    assert_eq!(removed.len(), 1);
    assert!(removed[0].contains("shape.name"));

    let retyped = compatible(&frozen, &Retyped::kind().describe()).expect_err("a field changed");
    assert_eq!(retyped.len(), 2);
    assert!(retyped[0].contains("shape.name"));
    assert!(retyped[1].contains("shape.style.width"));
}
