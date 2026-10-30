use block_editor_plugin::{AnchorSpace, HostTransform};

use super::*;

#[test]
fn a_zoomed_out_views_menu_paints_all_of_itself() {
    let _root = RootScope::new();
    let open_at = create_signal(None::<Pos2>);
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |_| {
            let items = view! {
                <ForEach keys={(0..2).collect::<Vec<usize>>()}>|index: usize| {
                    let label = format!("Item {index}");
                    view! {
                        <beui::unstyled::MenuItem label on_click={|| {}} />
                    }
                }</ForEach>
            };
            view! {
                <styled::ContextMenu
                    items
                    open_at={Prop::dynamic(move || open_at.get())}
                    on_close={move || open_at.set(None)}
                >
                    <styled::Frame />
                </styled::ContextMenu>
            }
        }),
    );
    let mut shown = whole(200.0, 120.0);
    shown.raster = 0.5;
    shown.anchor_space = Some(AnchorSpace {
        to_host: HostTransform {
            origin: pos2(100.0, 100.0),
            scale: Vec2::splat(0.5),
        },
        bounds: Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0)),
    });
    one(&mut surface, &shown);
    one(&mut surface, &shown);
    open_at.set(Some(pos2(40.0, 40.0)));
    let frame = one(&mut surface, &shown);
    let menu = anchored_shown(&shown, &frame.anchored[0]);
    let framed = one_anchored(&mut surface, &menu);
    assert_eq!(
        framed.painted,
        vec![menu.rect],
        "the host draws all of the menu, its outline included"
    );
    let painting = surface.painting(menu.id()).expect("the view is painted");
    assert_eq!(
        painting.frame.size,
        [202, 68],
        "at the app window's density, whatever the region's zoom"
    );
}
