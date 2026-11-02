use accesskit::{Role, Toggled};

use super::*;
use crate::reactive::*;
use crate::styled::*;

fn radios(harness: &mut Harness) -> Vec<(String, Option<Toggled>)> {
    let output = harness.frame(Vec::new());
    let tree = output.accessibility_tree("Test", VIEWPORT);
    let mut radios: Vec<_> = tree
        .nodes
        .iter()
        .filter(|(_, node)| node.role() == Role::MenuItemRadio)
        .map(|(_, node)| (node.label().unwrap_or_default().to_owned(), node.toggled()))
        .collect();
    radios.sort();
    radios
}

#[test]
fn a_submenu_holding_the_chosen_radio_reads_as_chosen() {
    let _root = crate::reactive::RootScope::new();
    let chosen = create_signal(1usize);
    let document = build_node(move || {
        let choice = move |at: usize, label: &str| {
            let action = Action::new(format!("choice.{at}"), label, move || chosen.set(at))
                .radio(create_memo(move || chosen.get() == at))
                .detached();
            MenuNode::action(&action)
        };
        let ends = vec![choice(1, "At the start"), choice(2, "At the end")];
        let nodes = vec![
            choice(0, "In the middle"),
            MenuNode::submenu("ends", "At an end", None, ends),
        ];
        view! {
            <MenuRows active=true items={view! {
                <crate::unstyled::MenuNodes nodes={nodes} />
            }} />
        }
    });
    let mut harness = Harness::new(document);
    assert_eq!(
        radios(&mut harness),
        vec![
            ("At an end".to_owned(), Some(Toggled::True)),
            ("In the middle".to_owned(), Some(Toggled::False)),
        ],
        "the submenu row holding the chosen place reads as chosen"
    );

    chosen.set(0);
    assert_eq!(
        radios(&mut harness),
        vec![
            ("At an end".to_owned(), Some(Toggled::False)),
            ("In the middle".to_owned(), Some(Toggled::True)),
        ],
        "and as not chosen once the choice is outside it"
    );
}
