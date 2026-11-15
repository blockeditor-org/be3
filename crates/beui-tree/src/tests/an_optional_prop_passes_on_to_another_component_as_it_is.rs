use super::*;

#[component]
fn Inner(#[prop(default = None)] label: Option<Prop<String>>) -> DomNode {
    let text = match label {
        Some(label) => label.get_untracked(),
        None => "none".to_owned(),
    };
    view! {
        <Element tag="p" text={text} />
    }
}

#[component]
fn Outer(#[prop(default = None)] label: Option<Prop<String>>) -> DomNode {
    view! {
        <Inner label />
    }
}

#[test]
fn an_optional_prop_passes_on_to_another_component_as_it_is() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    assert!(view! { <Outer /> }.html().contains(">none</p>"));
    assert!(view! { <Outer label="given" /> }.html().contains(">given</p>"));
}
