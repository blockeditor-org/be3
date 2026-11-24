use super::*;

#[test]
fn a_preview_shows_its_editors_preview_view_and_keeps_its_editor_once_promoted() {
    let place = rect((10.0, 20.0), (110.0, 70.0));

    // A preview asks its editor for its preview view the size of its place, in no frame, at its
    // placer's raster.
    let mut preview = placed(place, ChildMode::Preview);
    preview.own_frame = true;
    let screen = preview_screen(&preview, EditorInstanceId(3), 2.0);
    assert_eq!(screen.screen, PREVIEW_SCREEN);
    assert_eq!(screen.view, ViewId::PREVIEW);
    assert_eq!(screen.placement, ViewPlacement::Parent(ParentView::Preview));
    assert_eq!(screen.frame, None);
    assert_eq!(
        (screen.metrics.pixel_width, screen.metrics.pixel_height),
        (200, 100)
    );
    // A large one is painted at no more than its cap along its longer side.
    let large = placed(rect((0.0, 0.0), (4096.0, 1024.0)), ChildMode::Preview);
    let screen = preview_screen(&large, EditorInstanceId(3), 2.0);
    assert_eq!(
        (screen.metrics.pixel_width, screen.metrics.pixel_height),
        (2048, 512)
    );

    // While its editor opens it says it shows its preview view, which its placer leaves a hole
    // for once available; it takes no input.
    let child = wanted(placed(place, ChildMode::Preview));
    assert_eq!(child.screen(), PREVIEW_SCREEN);
    assert_eq!(child.shown_preview(), ShownPreview::View);
    assert!(!child.takes_input(&HashSet::default()));
    // One whose editor draws no preview shows its label, which its placer draws.
    *child.showing.borrow_mut() = Showing::Label;
    assert_eq!(child.shown_preview(), ShownPreview::Label);
    // A child placed in a preview view is a preview too, as under be-host, unless hidden.
    let live = placed(place, ChildMode::Live);
    assert_eq!(in_view(&live, ViewId::PREVIEW).mode, ChildMode::Preview);
    assert_eq!(in_view(&live, ViewId::MAIN).mode, ChildMode::Live);
    let hidden = placed(place, ChildMode::Hidden);
    assert_eq!(in_view(&hidden, ViewId::PREVIEW).mode, ChildMode::Hidden);
    // A child placed to run its editor is not a preview.
    let running = wanted(placed(place, ChildMode::Active));
    assert_eq!(running.screen(), CHILD_SCREEN);
    assert_eq!(running.shown_preview(), ShownPreview::None);

    // Promoted from its preview, as a canvas does a card it focuses, it keeps its editor, which
    // is asked for its main view.
    let previewed = wanted(placed(place, ChildMode::Preview));
    let promoted = reconcile(
        vec![previewed],
        vec![wanted(placed(place, ChildMode::Active))],
        &[],
    );
    assert!(promoted.opened.is_empty());
    assert!(promoted.gone.is_empty());
    assert_eq!(promoted.kept[0].screen(), CHILD_SCREEN);

    // One shown as its label is opened afresh once promoted, since no editor runs for it.
    let labelled = wanted(placed(place, ChildMode::Preview));
    *labelled.showing.borrow_mut() = Showing::Label;
    let promoted = reconcile(
        vec![labelled],
        vec![wanted(placed(place, ChildMode::Active))],
        &[],
    );
    assert_eq!(promoted.opened, vec![0]);
}
