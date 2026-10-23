use super::*;
use crate::reactive::*;

#[test]
fn a_timer_asks_for_frames_until_its_work_settles() {
    let _root = crate::reactive::RootScope::new();
    let runs = create_signal(0);
    let document = build_node(move || {
        let timer = create_timer(move || {
            runs.update(|runs| *runs += 1);
            (runs.get_untracked() < 3).then_some(Duration::ZERO)
        });
        timer.start(Duration::ZERO);
        view! {
            <List spacing=0.0>
                <Frame height=10.0 />
            </List>
        }
    });
    let mut harness = Harness::new(document);

    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::ZERO);
    assert_eq!(runs.get_untracked(), 1);
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::ZERO);
    assert_eq!(runs.get_untracked(), 2);
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);
    assert_eq!(runs.get_untracked(), 3);
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);
    assert_eq!(
        runs.get_untracked(),
        3,
        "a timer whose work settled runs no more"
    );
}
