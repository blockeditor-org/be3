use super::*;

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "shape")]
struct Before {
    line_width: f32,
    colour: String,
}

fn doubled(width: f32) -> f32 {
    width * 2.0
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "shape")]
struct After {
    #[model(from_old("line_width", doubled))]
    stroke: f32,
    #[model(alias = "colour")]
    color: String,
}

#[test]
fn a_renamed_property_still_reads_its_old_name() {
    let before = Document::new(&Before {
        line_width: 1.5,
        colour: "red".to_owned(),
    });

    let after = Document::<After>::from_bytes(&before.to_bytes()).expect("the bytes decode");
    assert_eq!(
        after.root(),
        After {
            stroke: 3.0,
            color: "red".to_owned(),
        }
    );

    let saved = after.to_stored().into_map().expect("a document");
    let root = saved
        .into_iter()
        .find(|(key, _)| key.as_text() == Some("root"))
        .and_then(|(_, root)| root.into_map().ok())
        .expect("a root");
    let names: Vec<&str> = root.iter().filter_map(|(key, _)| key.as_text()).collect();
    assert_eq!(names, ["$kind", "color", "stroke"]);
}
