use super::*;
use crate::selection::client;

#[test]
fn a_file_taken_back_outside_leaves_the_window_without_a_drop() {
    let mut harness = Harness::new();
    let _pointer = harness.client.pointer();
    let _device = harness.client.data_device();
    let (window, id) = harness.open();
    let center = harness.window(id).center();

    harness
        .stepped
        .ask(&["move", &format!("{},{}", center.x, center.y)])
        .expect("the pointer moves");
    harness
        .stepped
        .ask(&["grab", "/home/me/a.txt"])
        .expect("a file is grabbed");
    harness.settle();
    assert_eq!(client::drop_target().as_ref(), Some(&window.surface));

    harness.stepped.ask(&["ungrab"]).expect("the file is taken back");
    harness.settle();
    assert_eq!(client::drop_target(), None, "the drag leaves the window");
    assert_eq!(client::dropped_on(), None, "nothing is dropped");
}
