use super::*;
use crate::Stored;

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "sheet")]
struct Flat {
    cells: String,
}

fn split(format: u32, root: &mut Stored) {
    if format >= 2 {
        return;
    }
    let Stored::Map(entries) = root else {
        return;
    };
    for (key, value) in entries.iter_mut() {
        if key.as_text() == Some("cells") {
            *key = Stored::from("first");
            let text = value.as_text().unwrap_or_default().to_owned();
            *value = Stored::from(text.split(',').next().unwrap_or_default());
        }
    }
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
#[model(kind = "sheet", format = 2, migrate = split)]
struct Split {
    first: String,
}

#[test]
fn an_older_format_is_migrated_and_a_newer_one_is_refused() {
    let flat = Document::new(&Flat {
        cells: "a,b".to_owned(),
    });

    let split = Document::<Split>::from_bytes(&flat.to_bytes()).expect("the bytes decode");
    assert_eq!(split.root().first, "a");

    assert!(Document::<Flat>::from_bytes(&split.to_bytes()).is_err());
}
