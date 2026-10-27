use super::*;
use beui_macros::view;

#[test]
fn a_styled_dialog_on_the_html_core() {
    painting([480, 320], || {
        view! {
            <Root>
                <Dialog
                    open=true
                    title="Delete the block?"
                    kind=DialogKind::Alert
                    actions={vec![DialogAction::primary("Delete", || {}), DialogAction::cancel("Cancel")]}
                >
                    <Text>"Its history goes with it."</Text>
                </Dialog>
            </Root>
        }
    })
    .snapshot("html_styled_dialog.browser");
}
