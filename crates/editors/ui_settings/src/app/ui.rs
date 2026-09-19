use block_client::blocks::ui_settings::{UiSettings, UiSettingsOperation};
use block_editor_plugin::Editor;
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::{
    Align, Direction, Frame, ItemSize, List, clone, component, create_memo, view,
};
use block_editor_plugin::beui::styled::{Body, Caption, Slider, use_theme};

const PADDING: f32 = 20.0;
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 3.0;
const VALUE_WIDTH: f32 = 56.0;

#[component]
pub fn UiSettingsView(editor: Editor) -> NodeId {
    let settings = editor.block::<UiSettings>();
    let zoom = settings.project(UiSettings::zoom);
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
                        min=MIN_ZOOM
                        max=MAX_ZOOM
                        label="Zoom"
                        @test_id={"ui-settings.zoom"}
                        on_change={move |zoom| settings.operate(UiSettingsOperation::SetZoom { zoom })}
                    />
                    <Body @sizing=ItemSize::Fixed(VALUE_WIDTH) content={shown} />
                </List>
            </List>
        </Frame>
    }
}
