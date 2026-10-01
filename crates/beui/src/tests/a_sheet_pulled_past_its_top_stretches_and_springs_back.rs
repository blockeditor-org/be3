use super::*;
use crate::reactive::{NodeRef, with_reactive_scope};
use crate::styled::ModalSheet;

#[test]
fn a_sheet_pulled_past_its_top_stretches_and_springs_back() {
    let sheet = NodeRef::new();
    let (open, set_open) = create_signal(false);
    let document = build({
        let sheet = sheet.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Frame @sizing=ItemSize::Percent(100.0) />
                    <ModalSheet @node_ref=&sheet open={open} fit=true on_close={|| ()}>
                        <Frame height=120.0 />
                    </ModalSheet>
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    harness.frame(Vec::new());
    let handle = harness.find("sheet.handle");
    let fitted = harness.rect(handle).top();

    let from = pos2(200.0, fitted + 60.0);
    harness.touch(TouchPhase::Start, from);
    for step in 1..=20 {
        harness.touch(TouchPhase::Move, from - vec2(0.0, step as f32 * 10.0));
    }
    let stretched = harness.rect(handle).top();
    assert!(
        stretched < fitted - 20.0 && stretched > fitted - 200.0,
        "pulled up past where it fits, the sheet stretches a little way, \
         to {stretched} from {fitted}"
    );

    harness.touch(TouchPhase::End, from - vec2(0.0, 200.0));
    harness.frame(Vec::new());
    let springing = harness.rect(handle).top();
    assert!(
        springing > stretched && springing < fitted,
        "let go, it springs back down rather than jumping, and is at {springing}"
    );
    harness.settle();
    assert_eq!(
        harness.rect(handle).top(),
        fitted,
        "and comes to rest where it fits"
    );
}
