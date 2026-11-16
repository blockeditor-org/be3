use super::*;
use crate::reactive::{Binding, Build, Callback, ChildValue, Func, RenderFn};

// Each prop's last type argument is a type parameter of its own component that no other
// prop names, so what is passed decides it.

#[component]
fn Calls<T: Default + 'static>(on_change: Callback<T>) -> DomNode {
    on_change.call(T::default());
    DomNode::new("calls")
}

#[component]
fn Maybe<T: std::fmt::Debug>(value: Option<T>) -> DomNode {
    let node = DomNode::new("maybe");
    *node.0.text.borrow_mut() = format!("{value:?}");
    node
}

#[component]
fn Bound<T: Clone + std::fmt::Debug + 'static>(value: Binding<T>) -> DomNode {
    let node = DomNode::new("bound");
    *node.0.text.borrow_mut() = format!("{:?}", value.get_untracked());
    node
}

#[component]
fn Gathered<C: SlotChild>(children: Children<C>) -> Children<C> {
    children
}

#[component]
fn Maps<T: From<u8> + std::fmt::Debug + 'static>(map: Func<T, T>) -> DomNode {
    let node = DomNode::new("maps");
    *node.0.text.borrow_mut() = format!("{:?}", map.call(T::from(1)));
    node
}

#[component]
fn Header<C: ChildValue + 'static>(header: Option<RenderFn<u32, C>>) -> Option<C> {
    header.map(|header| header.call(7))
}

#[component]
fn After<C: ChildValue>(after: Option<Build<C>>) -> Option<C> {
    after.map(Build::call)
}

#[test]
fn a_prop_whose_type_comes_from_what_is_passed_takes_what_its_type_takes() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let called = create_signal(false);
    let picked = create_signal(3_u32);
    let page = view! {
        <Element tag="section">
            <Calls on_change={move |_: u32| called.set(true)} />
            <Maybe value=5_u32 />
            <Bound value={picked} />
            <Gathered>
                <Element tag="i" />
            </Gathered>
            <Maps map={|value: u32| value + 1} />
            <Header header={|value: u32| view! { <Element tag="b" text={value.to_string()} /> }} />
            <After after={|| view! { <Element tag="u" /> }} />
        </Element>
    };
    let html = page.html();
    assert!(called.get_untracked());
    assert!(html.contains(">Some(5)</maybe>"), "{html}");
    assert!(html.contains(">3</bound>"), "{html}");
    assert!(html.contains("<i id="), "{html}");
    assert!(html.contains(">2</maps>"), "{html}");
    assert!(html.contains(">7</b>"), "{html}");
    assert!(html.contains("<u id="), "{html}");
}
