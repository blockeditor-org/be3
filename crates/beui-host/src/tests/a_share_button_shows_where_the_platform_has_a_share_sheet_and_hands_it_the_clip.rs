use super::*;
use crate::Clip;
use crate::native::testing::{Showing, Stepped};
use beui_core::window::WindowSize;

#[test]
fn a_share_button_shows_where_the_platform_has_a_share_sheet_and_hands_it_the_clip() {
    let _root = RootScope::new();
    let document = build_node(move || {
        let sheet = use_share_sheet();
        view! {
            <List spacing=0.0>
                <Show value={sheet}>||
                    <Button
                        @test_id={"share"}
                        label="Share"
                        on_click={|| share(Clip::text("https://be3.example/#invite/CODE"))}
                    />
                </Show>
            </List>
        }
    });
    let window = WindowSize {
        size: VIEWPORT,
        scale: 1.0,
    };
    let mut program = Stepped::new(context(), Showing(document), window, None);
    program.frame(Vec::new());
    assert!(
        !program.shown("share"),
        "a headless window has no share sheet, so nothing offers one"
    );

    program
        .simulation()
        .expect("a simulated window")
        .share_sheet = true;
    program.frame(Vec::new());
    program.frame(Vec::new());
    assert!(program.shown("share"), "a platform with a sheet shows Share");
    program.click("share");
    let shared = &program.simulation().expect("a simulated window").shared;
    assert_eq!(
        shared
            .iter()
            .map(|clip| clip.plain_text())
            .collect::<Vec<_>>(),
        [Some("https://be3.example/#invite/CODE".to_owned())],
        "the press hands the clip to the sheet, once"
    );
}
