use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[derive(Clone)]
struct Accent(Color32);

#[component]
fn Provider(seen: WriteSignal<Option<Color32>>) -> NodeId {
    provide_context(Accent(Color32::WHITE));
    view! {
        <LayoutBuilder>|_size: ReadSignal<Vec2>| {
            seen.set(use_context::<Accent>().map(|Accent(color)| color));
            view! {
                <Frame height=20.0 />
            }
        }</LayoutBuilder>
    }
}

#[test]
fn a_layout_builders_content_reads_the_context_its_caller_provides() {
    let _root = crate::reactive::RootScope::new();
    let (seen, set_seen) = create_signal(None);
    let document = build({
        move || {
            view! {
                <Provider seen={set_seen} />
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(seen.get_untracked(), Some(Color32::WHITE));
}
