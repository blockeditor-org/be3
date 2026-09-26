use beui::NodeId;
use beui::icons::{ICON_PLAY_ARROW, ICON_STOP, ICON_TERMINAL};
use beui::reactive::{
    Align, Direction, ForEach, ItemSize, List, Show, clone, component, create_memo, view,
};
use beui::styled::{
    Body, Button, ButtonVariant, Code, Icon, Select, Spinner, TextInput, use_theme,
};
use beui::unstyled::ChoiceOption;

use crate::model::{CUSTOM, Model, PRESETS};
use crate::pane::TerminalPane;

const SPACING: f32 = 8.0;
const PICKER_SHARE: f32 = 40.0;

#[component]
pub(crate) fn RunPanel(model: Model) -> NodeId {
    let theme = use_theme();
    let head = create_memo(clone!(model -> move || {
        let summary = model.head_summary.get();
        if summary.is_empty() {
            "Reading the checkout".to_owned()
        } else {
            format!("Checked out: {summary}")
        }
    }));
    let custom = create_memo(clone!(model -> move || model.preset.get() == CUSTOM));
    let preview = create_memo(clone!(model -> move || {
        let args = match PRESETS.get(model.preset.get()) {
            Some(preset) if preset.args.is_empty() => model.custom.get(),
            Some(preset) => preset.args.to_owned(),
            None => String::new(),
        };
        format!("./scripts/buck {args}")
    }));
    let named = create_memo(clone!(custom -> move || !custom.get()));
    let running = model.running.clone();
    let cannot_run = create_memo(clone!(model running -> move || {
        running.get()
            || (model.preset.get() == CUSTOM && model.custom.get().trim().is_empty())
    }));
    let idle = create_memo(clone!(running -> move || !running.get()));
    let set_preset = model.set_preset.clone();
    let set_custom = model.set_custom.clone();
    let run = clone!(model -> move || model.run());
    let submit = clone!(model -> move |_: String| model.run());
    let stop = clone!(model -> move || model.stop());
    let muted = theme.text_muted.clone();
    let selected = create_memo(clone!(model -> move || Some(model.preset.get())));
    let custom_value = model.custom.clone();
    let pane = model.pane.clone();
    view! {
        <List spacing=SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Icon glyph=ICON_TERMINAL color={muted} />
                <Body @sizing=ItemSize::Percent(100.0) content={head} />
                <Show condition={running.clone()}>
                    <Spinner width=18.0 label="A command is running" />
                </Show>
            </List>
            <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                <Select
                    @sizing=ItemSize::Percent(PICKER_SHARE)
                    options={view! {
                        <ForEach keys={(0..PRESETS.len()).collect::<Vec<_>>()}>
                            {|index: usize| view! {
                                <ChoiceOption label={PRESETS[index].title} />
                            }}
                        </ForEach>
                    }}
                    selected
                    label="Command to run"
                    on_change={move |selected: Option<usize>| {
                        if let Some(index) = selected {
                            set_preset.set(index);
                        }
                    }}
                />
                <Show condition={named}>
                    <Code
                        @sizing=ItemSize::Percent(100.0)
                        content={preview}
                        color={theme.text_muted.clone()}
                    />
                </Show>
                <Show condition={custom}>
                    <TextInput
                        @sizing=ItemSize::Percent(100.0)
                        value={custom_value}
                        placeholder="Arguments for ./scripts/buck, like build //crates/beui:demo-example"
                        label="Custom ./scripts/buck arguments"
                        on_change={move |value| set_custom.set(value)}
                        on_submit={submit}
                    />
                </Show>
                <Button
                    label="Run"
                    glyph=ICON_PLAY_ARROW
                    variant=ButtonVariant::Primary
                    disabled={cannot_run}
                    on_click={run}
                />
                <Button
                    label="Stop"
                    glyph=ICON_STOP
                    variant=ButtonVariant::Secondary
                    disabled={idle}
                    on_click={stop}
                />
            </List>
            <TerminalPane @sizing=ItemSize::Percent(100.0) pane />
        </List>
    }
}
