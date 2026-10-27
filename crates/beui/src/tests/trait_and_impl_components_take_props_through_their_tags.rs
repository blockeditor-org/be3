use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[component]
trait Theme: 'static {
    #[component]
    fn Card(title: String, #[prop(default = 1)] count: usize) -> NodeId;

    #[component]
    fn Badge(label: String) -> NodeId {
        view! {
            <Text>{format!("badge {label}")}</Text>
        }
    }
}

struct Plain;

#[component]
impl Theme for Plain {
    #[component]
    fn Card(title: String, count: usize) -> NodeId {
        view! {
            <Text>{format!("{title} x{count}")}</Text>
        }
    }
}

struct Shelf;

#[component]
impl Shelf {
    #[component]
    fn Row(#[prop(default = "row".to_string())] name: String) -> NodeId {
        view! {
            <Text>{name}</Text>
        }
    }
}

fn shelf<T: Theme>() -> NodeId {
    view! {
        <List spacing=0.0>
            <T::Card title="first" />
            <T::Card title="second" count=2 />
            <T::Badge label="new" />
            <Shelf::Row />
            <Shelf::Row name="named" />
        </List>
    }
}

#[test]
fn trait_and_impl_components_take_props_through_their_tags() {
    let document = build_node(shelf::<Plain>);
    let mut driven = stepped(crate::native::testing::Showing(document));
    let components = driven
        .ask(&["comps"])
        .expect("the component tree is answered through a frame");
    let module = format!("{}::", module_path!());
    let names: Vec<&str> = components
        .lines()
        .filter_map(|line| line.trim().strip_prefix(module.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            "Plain::Card",
            "Plain::Card",
            "Theme::Badge",
            "Shelf::Row",
            "Shelf::Row"
        ],
        "{components}"
    );
    for text in ["first x1", "second x2", "badge new", "row", "named"] {
        assert!(
            components.contains(&format!("\"{text}\"")),
            "{text} in {components}"
        );
    }
}
