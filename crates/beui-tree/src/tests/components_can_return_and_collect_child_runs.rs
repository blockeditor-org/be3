use super::*;

#[component]
fn Siblings(label: Prop<String>, visible: Prop<bool>) -> Children<DomNode> {
    view! {
        <Element tag="p" text={label} />
        <Show condition={visible}>
            <Element tag="span" text="tail" />
        </Show>
    }
}

#[component]
fn Optional(visible: bool) -> Option<DomNode> {
    visible.then(|| {
        view! {
            <Element tag="aside" text="optional" />
        }
    })
}

#[test]
fn components_can_return_and_collect_child_runs() {
    let _root = ::reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let label = create_signal("one".to_string());
    let visible = create_signal(false);
    let page = scope.run(|| {
        let groups: Children<DomNode> = [view! {
            <Siblings label={label} visible={visible} />
        }]
        .into_iter()
        .collect();
        view! {
            <Element tag="div">{groups}</Element>
        }
    });
    assert!(page.html().contains(">one</p>"));
    assert_eq!(page.0.children.borrow().len(), 1);
    visible.set(true);
    label.set("two".to_string());
    assert!(page.html().contains(">two</p>"));
    assert!(page.html().contains(">tail</span>"));
    assert_eq!(page.0.children.borrow().len(), 2);
    visible.set(false);
    assert_eq!(page.0.children.borrow().len(), 1);
    assert!(
        scope
            .run(|| view! {
                <Optional visible=false />
            })
            .is_none()
    );
    assert!(
        scope
            .run(|| view! {
                <Optional visible=true />
            })
            .is_some()
    );
}
