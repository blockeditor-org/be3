use super::*;

#[component]
fn ReadsInItsBody(count: Signal<usize>) -> DomNode {
    view! {
        <Element tag="p" text={count.get().to_string()} />
    }
}

#[cfg(debug_assertions)]
#[test]
fn a_tracked_read_in_a_component_body_panics_naming_its_type() {
    let _root = ::reactive::RootScope::new();
    let scope = Scope::new();
    let count = create_signal(1_usize);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scope.run(|| {
            view! {
                <ReadsInItsBody count={count} />
            }
        })
    }))
    .err()
    .expect("a tracked read in a component body panics");
    let message = panic
        .downcast_ref::<String>()
        .expect("the panic carries a formatted message");
    assert!(message.contains("`Signal<usize>`"), "{message}");
    assert!(message.contains("closure, memo or effect"), "{message}");
    assert!(
        message.contains("signal or memo itself as the prop"),
        "{message}"
    );
    assert!(message.contains("get_untracked()"), "{message}");
    assert_eq!(count.get(), 1);
}
