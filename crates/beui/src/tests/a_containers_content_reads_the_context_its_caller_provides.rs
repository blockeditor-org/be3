use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Container;

#[derive(Clone)]
struct Accent(Color32);

#[component]
fn Provider(seen: Signal<Option<Color32>>) -> NodeId {
    provide_context(Accent(Color32::WHITE));
    view! {
        <Container>|_size: ReadSignal<Vec2>| {
            seen.set(use_context::<Accent>().map(|Accent(color)| color));
            view! {
                <Frame height=20.0 />
            }
        }</Container>
    }
}

#[test]
fn a_containers_content_reads_the_context_its_caller_provides() {
    let _root = crate::reactive::RootScope::new();
    let seen = create_signal(None);
    let document = build_node(move || {
        view! {
            <Provider seen={seen} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(seen.get_untracked(), Some(Color32::WHITE));
}
