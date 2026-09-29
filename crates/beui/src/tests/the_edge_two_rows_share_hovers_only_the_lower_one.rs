use super::*;
use crate::reactive::{ClickCatcher, Frame, List, build, view};

const ROW: f32 = 20.0;

#[test]
fn the_edge_two_rows_share_hovers_only_the_lower_one() {
    let upper = Rc::new(Cell::new(false));
    let lower = Rc::new(Cell::new(false));
    let document = build({
        let (upper, lower) = (upper.clone(), lower.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <ClickCatcher on_hover_change={move |hovered| upper.set(hovered)}>
                        <Frame height=ROW />
                    </ClickCatcher>
                    <ClickCatcher on_hover_change={move |hovered| lower.set(hovered)}>
                        <Frame height=ROW />
                    </ClickCatcher>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.frame(vec![Event::PointerMoved(pos2(10.0, ROW - 0.5))]);
    assert!(upper.get() && !lower.get());

    harness.frame(vec![Event::PointerMoved(pos2(10.0, ROW))]);
    assert!(
        !upper.get() && lower.get(),
        "a pointer on the edge between two rows hovers one of them"
    );
}
