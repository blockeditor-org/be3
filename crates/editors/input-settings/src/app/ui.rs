use block_editor_beui::Editor;
use block_editor_beui::be_block::input_settings::{
    MAX_POINTER_SPEED, MAX_REPEAT_DELAY, MAX_REPEAT_RATE, MIN_POINTER_SPEED, MIN_REPEAT_DELAY,
    MIN_REPEAT_RATE,
};
use block_editor_beui::be_block::{InputSettings, InputSettingsContent, ObjectId};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{
    Align, Direction, Frame, ItemSize, List, clone, component, create_memo, view,
};
use block_editor_beui::beui::styled::{
    Body, Caption, Heading, Scroll, Slider, Switch, TextInput, use_theme,
};

const PADDING: f32 = 20.0;
const VALUE_WIDTH: f32 = 72.0;
const SECTION_SPACING: f32 = 18.0;
const ROW_SPACING: f32 = 8.0;
const LABEL_SPACING: f32 = 4.0;
const COLUMN_SPACING: f32 = 12.0;

#[component]
pub fn InputSettingsView(editor: Editor) -> NodeId {
    let settings = editor.block_content::<InputSettingsContent>();
    let read_only = editor.read_only();
    let layout = settings.field(ObjectId::ROOT, InputSettings::KEYBOARD_LAYOUT);
    let variant = settings.field(ObjectId::ROOT, InputSettings::KEYBOARD_VARIANT);
    let options = settings.field(ObjectId::ROOT, InputSettings::KEYBOARD_OPTIONS);
    let delay = settings.field(ObjectId::ROOT, InputSettings::REPEAT_DELAY);
    let delay = create_memo(move || delay.get().milliseconds() as f32);
    let rate = settings.field(ObjectId::ROOT, InputSettings::REPEAT_RATE);
    let rate = create_memo(move || rate.get().per_second() as f32);
    let speed = settings.field(ObjectId::ROOT, InputSettings::POINTER_SPEED);
    let speed = create_memo(move || speed.get().get());
    let tap = settings.field(ObjectId::ROOT, InputSettings::TAP_TO_CLICK);
    let tap = create_memo(move || tap.get().0);
    let natural = settings.field(ObjectId::ROOT, InputSettings::NATURAL_SCROLL);
    let natural = create_memo(move || natural.get());
    let delay_shown = create_memo(clone!(delay -> move || format!("{} ms", delay.get())));
    let rate_shown = create_memo(clone!(rate -> move || format!("{} / s", rate.get())));
    let speed_shown = create_memo(clone!(speed -> move || format!("{:+.2}", speed.get())));
    let (layout_off, variant_off, options_off) =
        (read_only.clone(), read_only.clone(), read_only.clone());
    let (delay_off, rate_off, speed_off) = (read_only.clone(), read_only.clone(), read_only);
    let (set_layout, set_variant, set_options) =
        (settings.clone(), settings.clone(), settings.clone());
    let (set_delay, set_rate, set_speed) = (settings.clone(), settings.clone(), settings.clone());
    let set_tap = settings.clone();
    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()}>
            <Scroll>
                <Frame padding_horizontal=PADDING padding_vertical=PADDING>
                    <List spacing=SECTION_SPACING>
                        <List spacing=ROW_SPACING>
                            <Heading content="Keyboard" />
                            <Caption
                                content="XKB names, like us, de or us,de. Empty uses the default."
                            />
                            <TextInput
                                value={layout}
                                label="Layout"
                                placeholder="Layout"
                                disabled={layout_off}
                                @test_id={"input-settings.layout"}
                                on_change={move |layout: String| {
                                    set_layout.operate(InputSettings::set_keyboard_layout(&layout))
                                }}
                            />
                            <TextInput
                                value={variant}
                                label="Variant"
                                placeholder="Variant"
                                disabled={variant_off}
                                @test_id={"input-settings.variant"}
                                on_change={move |variant: String| {
                                    set_variant.operate(InputSettings::set_keyboard_variant(&variant))
                                }}
                            />
                            <TextInput
                                value={options}
                                label="Options"
                                placeholder="Options, like caps:escape"
                                disabled={options_off}
                                @test_id={"input-settings.options"}
                                on_change={move |options: String| {
                                    set_options.operate(InputSettings::set_keyboard_options(&options))
                                }}
                            />
                            <Caption content="Repeat delay" />
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=COLUMN_SPACING
                            >
                                <Slider
                                    @sizing=ItemSize::Percent(100.0)
                                    value={delay}
                                    min={MIN_REPEAT_DELAY as f32}
                                    max={MAX_REPEAT_DELAY as f32}
                                    label="Repeat delay"
                                    disabled={delay_off}
                                    @test_id={"input-settings.repeat-delay"}
                                    on_change={move |delay: f32| {
                                        set_delay.operate(InputSettings::set_repeat_delay(delay.round() as u32))
                                    }}
                                />
                                <Body @sizing=ItemSize::Fixed(VALUE_WIDTH) content={delay_shown} />
                            </List>
                            <Caption content="Repeat rate" />
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=COLUMN_SPACING
                            >
                                <Slider
                                    @sizing=ItemSize::Percent(100.0)
                                    value={rate}
                                    min={MIN_REPEAT_RATE as f32}
                                    max={MAX_REPEAT_RATE as f32}
                                    label="Repeat rate"
                                    disabled={rate_off}
                                    @test_id={"input-settings.repeat-rate"}
                                    on_change={move |rate: f32| {
                                        set_rate.operate(InputSettings::set_repeat_rate(rate.round() as u32))
                                    }}
                                />
                                <Body @sizing=ItemSize::Fixed(VALUE_WIDTH) content={rate_shown} />
                            </List>
                        </List>
                        <List spacing=ROW_SPACING>
                            <Heading content="Pointer" />
                            <List spacing=LABEL_SPACING>
                                <Caption content="Speed" />
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=COLUMN_SPACING
                                >
                                    <Slider
                                        @sizing=ItemSize::Percent(100.0)
                                        value={speed}
                                        min=MIN_POINTER_SPEED
                                        max=MAX_POINTER_SPEED
                                        label="Pointer speed"
                                        disabled={speed_off}
                                        @test_id={"input-settings.pointer-speed"}
                                        on_change={move |speed: f32| {
                                            set_speed.operate(InputSettings::set_pointer_speed(speed))
                                        }}
                                    />
                                    <Body
                                        @sizing=ItemSize::Fixed(VALUE_WIDTH)
                                        content={speed_shown}
                                    />
                                </List>
                            </List>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=COLUMN_SPACING
                            >
                                <Body @sizing=ItemSize::Percent(100.0) content="Tap to click" />
                                <Switch
                                    on={tap}
                                    label="Tap to click"
                                    @test_id={"input-settings.tap-to-click"}
                                    on_change={move |on: bool| {
                                        set_tap.operate(InputSettings::set_tap_to_click(on))
                                    }}
                                />
                            </List>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=COLUMN_SPACING
                            >
                                <Body
                                    @sizing=ItemSize::Percent(100.0)
                                    content="Natural scrolling"
                                />
                                <Switch
                                    on={natural}
                                    label="Natural scrolling"
                                    @test_id={"input-settings.natural-scroll"}
                                    on_change={move |on: bool| {
                                        settings.operate(InputSettings::set_natural_scroll(on))
                                    }}
                                />
                            </List>
                        </List>
                    </List>
                </Frame>
            </Scroll>
        </Frame>
    }
}
