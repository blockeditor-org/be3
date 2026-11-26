use super::*;

#[test]
fn a_focused_layer_that_went_loses_the_focus() {
    let mut windows = Windows::new(ROOT);
    windows.drew(drawn());
    windows.route(
        None,
        vec![button(150.0, true).into(), button(150.0, false).into()],
    );
    assert_eq!(windows.focused(None).map(|layer| layer.id), Some(SCREEN));
    let mut list = drawn();
    list.windows[0].layers.retain(|layer| layer.id != SCREEN);
    windows.drew(list);
    windows.drew(drawn());
    assert_eq!(
        windows.focused(None),
        None,
        "the screen lost the focus when it went, and does not have it back when it returns"
    );
}
