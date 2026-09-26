use block_editor_beui::Editor;
use block_editor_beui::be_block::{ObjectId, UiSettings, UiSettingsContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{
    Align, Direction, Frame, ItemSize, List, clone, component, create_memo, view,
};
use block_editor_beui::beui::styled::{Body, Caption, Slider, use_theme};

const PADDING: f32 = 20.0;
const VALUE_WIDTH: f32 = 56.0;

#[component]
pub fn UiSettingsView(editor: Editor) -> NodeId {
    let settings = editor.block_content::<UiSettingsContent>();
    let zoom = settings.field(ObjectId::ROOT, UiSettings::ZOOM);
    let zoom = create_memo(clone!(zoom -> move || zoom.get().get()));
    let shown = create_memo(clone!(zoom -> move || format!("{:.2}x", zoom.get())));
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=8.0>
                <Caption content="Zoom" />
                <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                    <Slider
                        @sizing=ItemSize::Percent(100.0)
                        value={zoom}
                        min=block_editor_beui::be_block::ui_settings::MIN_ZOOM
                        max=block_editor_beui::be_block::ui_settings::MAX_ZOOM
                        label="Zoom"
                        @test_id={"ui-settings.zoom"}
                        on_change={move |zoom| settings.operate(UiSettings::set_zoom(zoom))}
                    />
                    <Body @sizing=ItemSize::Fixed(VALUE_WIDTH) content={shown} />
                </List>
            </List>
        </Frame>
    }
}
