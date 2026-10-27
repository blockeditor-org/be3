use super::*;
use crate::{primitives, unstyled};
use beui_macros::view;

#[test]
fn a_long_modal_sheet_on_the_html_core() {
    painting([480, 320], || {
        view! {
            <Root>
                <unstyled::Modal
                    open=true
                    scrim={crate::Color32::from_rgba_unmultiplied(0, 0, 0, 96)}
                    placement=unstyled::ModalPlacement::Fill
                    motion=unstyled::ModalMotion::Still
                    on_dismiss={|| {}}
                >|_|
                    <List spacing=Space::None>
                        <primitives::Spacer @sizing=crate::ItemSize::Percent(100.0) />
                        <Frame
                            @sizing={crate::ItemSize::Intrinsic.shrink(1.0)}
                            fill=ColorRole::Surface
                            height=Size::Custom(200.0)
                        >
                            <Scroll>
                                <List spacing=Space::None>
                                    <ForEach keys={(1..=30).collect::<Vec<usize>>()}>|row: usize|
                                        <ActionRow label={format!("Row {row}")} on_click={|| {}} />
                                    </ForEach>
                                </List>
                            </Scroll>
                        </Frame>
                    </List>
                </unstyled::Modal>
            </Root>
        }
    })
    .snapshot("html_long_modal_sheet.browser");
}
