use super::*;
use crate::reactive::{LayoutBuilder, ReadSignal, build, component, provide_context, use_context, view};

#[derive(Clone)]
struct Accent(Color32);

#[component]
fn Provider(seen: Rc<Cell<Option<Color32>>>) -> NodeId {
    provide_context(Accent(Color32::WHITE));
    view! {
        <LayoutBuilder>
            {move |_size: ReadSignal<Vec2>| {
                seen.set(use_context::<Accent>().map(|Accent(color)| color));
                view! { <Frame height=20.0 /> }
            }}
        </LayoutBuilder>
    }
}

#[test]
fn a_layout_builders_content_reads_the_context_its_caller_provides() {
    let seen = Rc::new(Cell::new(None));
    let document = build({
        let seen = Rc::clone(&seen);
        move || view! { <Provider seen /> }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(seen.get(), Some(Color32::WHITE));
}
