use block_editor_beui::Editor;
use block_editor_beui::be_block::{Theme, UiSettings, UiSettingsContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{ForEach, Frame, List, component, create_memo, view};
use block_editor_beui::beui::styled::{Caption, Select, use_theme};
use block_editor_beui::beui::unstyled::ChoiceOption;

const PADDING: f32 = 20.0;

#[component]
pub fn UiSettingsView(editor: Editor) -> NodeId {
    let settings = editor.block_content::<UiSettingsContent>();
    let chosen = settings.project(|content| content.root().theme);
    let inherited = editor.inherited_settings::<UiSettings>();
    let inherited_label =
        create_memo(move || format!("Inherited ({})", inherited.get().theme().name()));
    let selected = create_memo(move || {
        Some(
            chosen
                .get()
                .and_then(|theme| Theme::ALL.iter().position(|known| *known == theme))
                .map_or(0, |index| index + 1),
        )
    });
    let read_only = editor.read_only();
    let theme = use_theme();
    view! {
        <Frame color={theme.background} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=8.0>
                <Caption content="Theme" />
                <Select
                    options={view! {
                        <ChoiceOption label={inherited_label} />
                        <ForEach keys={Theme::ALL.to_vec()}>
                            {|theme: Theme| view! {
                                <ChoiceOption label={theme.name().to_owned()} />
                            }}
                        </ForEach>
                    }}
                    selected={selected}
                    label="Theme"
                    disabled={read_only}
                    @test_id={"ui-settings.theme"}
                    on_change={move |index: Option<usize>| {
                        let theme = index
                            .and_then(|index| index.checked_sub(1))
                            .and_then(|index| Theme::ALL.get(index).copied());
                        settings.operate(UiSettings::set_theme(theme));
                    }}
                />
            </List>
        </Frame>
    }
}
