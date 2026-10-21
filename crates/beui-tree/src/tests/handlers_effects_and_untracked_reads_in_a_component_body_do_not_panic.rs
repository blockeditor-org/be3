use super::*;
use crate::reactive::{Callback, untrack};

#[component]
fn ReadsWhereItMay(count: Signal<usize>, label: Prop<String>, seen: Rc<Cell<usize>>) -> DomNode {
    let snapshot = count.get_untracked() + count.with_untracked(|count| *count);
    let untracked = untrack(|| count.get());
    let doubled = create_memo(move || count.get() * 2);
    let handler = Callback::new({
        let seen = seen.clone();
        move |()| seen.set(count.get())
    });
    handler.call(());
    create_effect(move || seen.set(seen.get() + doubled.get()));
    flush();
    view! {
        <Element
            tag="p"
            text={Prop::dynamic(move || format!("{}{snapshot}{untracked}", label.get()))}
        />
    }
}

#[test]
fn handlers_effects_and_untracked_reads_in_a_component_body_do_not_panic() {
    let _root = ::reactive::RootScope::new();
    fresh_ids();
    let scope = Scope::new();
    let count = create_signal(1_usize);
    let label = create_signal("n".to_string());
    let seen = Rc::new(Cell::new(0));
    let page = scope.run(|| {
        view! {
            <ReadsWhereItMay count={count} label={label} seen={seen.clone()} />
        }
    });
    assert_eq!(seen.get(), 3);
    assert!(page.html().contains(">n21</p>"));
    count.set(2);
    flush();
    assert_eq!(seen.get(), 7);
    label.set("m".to_string());
    flush();
    assert!(page.html().contains(">m21</p>"));
}
