use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Place(&'static str);

#[component]
fn Provides(place: &'static str, #[prop(children)] content: Build<NodeId>) -> NodeId {
    provide_context(Place(place));
    content.call()
}

#[test]
fn a_kept_subtree_reads_the_context_of_the_kept_that_shows_it() {
    let _root = crate::reactive::RootScope::new();
    let on_left = create_signal(true);
    let late = create_signal(false);
    let read = create_signal(Vec::<Option<Place>>::new());
    let document = build_node(move || {
        let panel = move || {
            view! {
                <Kept>|| {
                    read.update(|read| read.push(use_context::<Place>()));
                    view! {
                        <List spacing=0.0>
                            <Show value={late}>|| {
                                read.update(|read| read.push(use_context::<Place>()));
                                view! {
                                    <Frame />
                                }
                            }</Show>
                        </List>
                    }
                }</Kept>
            }
        };
        view! {
            <Provides place="around">||
                <KeepAlive>
                    <List spacing=0.0>
                        <Show value={on_left}>||
                            <Provides place="left">|| { panel() }</Provides>
                        </Show>
                        <Show value={move || !on_left.get()}>||
                            <Provides place="right">|| { panel() }</Provides>
                        </Show>
                    </List>
                </KeepAlive>
            </Provides>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(
        read.get_untracked(),
        vec![Some(Place("left"))],
        "built, it reads what is provided around the `Kept` that shows it"
    );

    on_left.set(false);
    late.set(true);
    harness.frame(Vec::new());
    assert_eq!(
        read.get_untracked().last(),
        Some(&Some(Place("right"))),
        "what it builds once moved reads the context of the `Kept` it moved to"
    );
}
