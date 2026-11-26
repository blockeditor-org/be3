use super::*;
use crate::reactive::BuildFn;

#[component]
fn Inner(label: Option<Prop<String>>, after: Option<BuildFn<DomNode>>) -> DomNode {
    let text = match label {
        Some(label) => label.get_untracked(),
        None => "none".to_owned(),
    };
    let page = view! {
        <Element tag="p" text={text} />
    };
    if let Some(after) = after {
        page.0.children.borrow_mut().push(after.call());
    }
    page
}

#[component]
fn Outer(label: Option<Prop<String>>, after: Option<BuildFn<DomNode>>) -> DomNode {
    view! {
        <Inner label after />
    }
}

#[test]
fn an_optional_prop_passes_on_to_another_component_as_it_is() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    assert!(
        view! {
            <Outer />
        }
        .html()
        .contains(">none</p>")
    );
    assert!(
        view! {
            <Outer label="given" />
        }
        .html()
        .contains(">given</p>")
    );
    let page = view! {
        <Outer
            after={|| view! {
                <Element tag="b" text="after" />
            }}
        />
    };
    assert!(page.html().contains(">after</b>"), "{}", page.html());
}
