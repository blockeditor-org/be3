use super::*;
use crate::reactive::*;
use crate::styled::*;

fn ranked(key: &str, title: &str, rank: u32) -> PaletteItem {
    PaletteItem {
        key: key.to_owned(),
        title: title.to_owned(),
        ranked: Some(rank),
        ..PaletteItem::default()
    }
}

#[test]
fn a_palette_keeps_the_order_its_source_ranked_over_its_own_scoring() {
    let _root = crate::reactive::RootScope::new();
    let heard = create_signal(Vec::<String>::new());
    let items = vec![
        ranked("gamma", "Gamma", 3),
        ranked("alpha", "Alpha", 2),
        ranked("beta", "Beta", 1),
    ];
    let document = build_node(move || {
        let open = create_signal(true);
        view! {
            <CommandPalette
                open
                items={PaletteItems::list(items)}
                score={PaletteItem::fuzzy_score as PaletteScore}
                id="hits"
                on_choose={move |key: String| heard.update(|heard| heard.push(key))}
            />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    harness.type_text("alpha");
    harness.frame(Vec::new());
    for key in ["gamma", "alpha", "beta"] {
        assert!(
            harness
                .document()
                .find_test_id(&format!("hits.item.{key}"))
                .is_some(),
            "a ranked item stays listed whatever is typed: {key}"
        );
    }
    harness.key(Key::Enter, Modifiers::NONE);
    assert_eq!(
        heard.get_untracked(),
        ["gamma"],
        "the source's first stays first, though the palette would score Alpha highest"
    );
}
