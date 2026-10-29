use super::*;
use crate::reactive::*;
use crate::styled::*;

fn item(key: &str, title: &str, terms: &[&str]) -> PaletteItem {
    PaletteItem {
        key: key.to_owned(),
        title: title.to_owned(),
        terms: terms.iter().map(|term| (*term).to_owned()).collect(),
        ..PaletteItem::default()
    }
}

#[test]
fn a_fuzzy_command_palette_filters_as_it_is_typed_and_chooses_with_enter() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::<String>::new());
    let items = vec![
        item("files", "Files", &["folder"]),
        item("foot", "Foot", &["terminal"]),
        item("term", "Terminal", &["shell"]),
    ];
    let document = build_node(move || {
        let open = create_signal(true);
        view! {
            <CommandPalette
                open
                items={PaletteItems::list(items)}
                score={PaletteItem::fuzzy_score as PaletteScore}
                id="programs"
                on_choose={move |key: String| heard.update(|heard| heard.push(format!("choose {key}")))}
            />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let shown = |harness: &Harness| {
        ["files", "foot", "term"]
            .into_iter()
            .filter(|key| {
                harness
                    .document()
                    .find_test_id(&format!("programs.item.{key}"))
                    .is_some()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(shown(&harness), ["files", "foot", "term"]);

    harness.type_text("term");
    harness.frame(Vec::new());
    assert_eq!(shown(&harness), ["foot", "term"]);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    assert_eq!(
        heard.get_untracked(),
        ["choose foot"],
        "the name ranks first, and the arrows move the highlight"
    );
}
