use super::*;
use crate::reactive::use_context;

fn provided_count() -> usize {
    use_context::<Signal<usize>>().map_or(0, |count| count.get())
}

#[component]
fn Counted(#[prop(default = provided_count())] count: usize) -> DomNode {
    view! {
        <Element tag="p" text={count.to_string()} />
    }
}

/// A default runs untracked, so a read in it is a snapshot rather than a tracked read in the
/// component's body, which panics.
#[test]
fn a_default_that_reads_a_signal_reads_it_once() {
    let _root = ::beui_reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let count = create_signal(4_usize);
    let page = scope.run(|| {
        crate::reactive::provide_context(count);
        view! {
            <Counted />
        }
    });
    assert!(page.html().contains(">4</p>"), "{}", page.html());
    count.set(5);
    assert!(page.html().contains(">4</p>"), "{}", page.html());
}
