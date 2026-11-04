//! A rule: a row that shows it as key caps, and opens into its editor.

use be_host_remap::edit::{Combo, Does, RuleEdit, RuleOutline, StepOutline, modifier_of};
use be_host_remap::names::Modifier;
use be_sdk_beui::beui::icons::{
    ICON_ARROW_RIGHT, ICON_CHEVRON_DOWN, ICON_CHEVRON_UP, ICON_COPY, ICON_PLUS, ICON_TIMER,
    ICON_TRASH, ICON_X,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::unstyled;
use be_sdk_beui::beui::{Align, Direction, ItemSize, NodeId, Overflow, TextAlign};

use super::gui::{Change, Edits};
use super::keys::label;
use super::picker::Target;

const ROW_PADDING: Padding = Padding::symmetric(Space::Xl, Space::Md);
const EDITOR_PADDING: Padding = Padding::symmetric(Space::Xl, Space::Lg);
const ROW_SPACING: Space = Space::Md;
const CAP_SPACING: Space = Space::Xs;
const PART_SPACING: Space = Space::Xl;
const LABEL_SPACING: Space = Space::Xs2;
const CONTROL_SPACING: Space = Space::Sm;
const MS_WIDTH: f32 = 88.0;
const HOLD_PRESETS: [(i64, &str); 4] = [
    (0, "Instant"),
    (20, "20 ms"),
    (50, "50 ms"),
    (100, "100 ms"),
];

/// A key cap as a row shows it: its label, and whether it only stands in for
/// a key (a wait).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Cap {
    label: String,
    muted: bool,
}

fn key_cap(name: &str) -> Cap {
    Cap {
        label: label(name),
        muted: false,
    }
}

/// What must be held for a rule, from `on` and its `held` list.
fn held(outline: &RuleOutline) -> Vec<String> {
    let on = Combo::parse(&outline.on);
    on.held
        .into_iter()
        .chain(outline.held.iter().cloned())
        .collect()
}

fn trigger_caps(outline: &RuleOutline) -> Vec<Cap> {
    let on = Combo::parse(&outline.on);
    held(outline)
        .iter()
        .map(|name| key_cap(name))
        .chain(std::iter::once(key_cap(&on.key)))
        .collect()
}

fn chord_caps(written: &str) -> Vec<Cap> {
    let chord = Combo::parse(written);
    chord
        .held
        .iter()
        .chain(std::iter::once(&chord.key))
        .map(|name| key_cap(name))
        .collect()
}

fn result_caps(outline: &RuleOutline) -> Vec<Cap> {
    match outline.does {
        Does::Send | Does::Tap => chord_caps(&outline.key),
        Does::Steps => outline
            .steps
            .iter()
            .map(|step| match step {
                StepOutline::Tap { key, .. } => Cap {
                    label: chord_caps(key)
                        .into_iter()
                        .map(|cap| cap.label)
                        .collect::<Vec<_>>()
                        .join(" + "),
                    muted: false,
                },
                StepOutline::Wait { ms } => Cap {
                    label: format!("wait {ms} ms"),
                    muted: true,
                },
            })
            .collect(),
    }
}

fn names(names: &[String]) -> String {
    names
        .iter()
        .map(|name| label(name))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The options set that a closed row or a closed "More options" mentions.
fn options(outline: &RuleOutline, held_for: bool) -> Vec<String> {
    let mut options = Vec::new();
    if held_for && outline.does == Does::Tap && outline.hold_ms.unwrap_or(0) > 0 {
        options.push(format!("held {} ms", outline.hold_ms.unwrap_or(0)));
    }
    if !outline.not_held.is_empty() {
        options.push(format!("not with {}", names(&outline.not_held)));
    }
    if outline.keep {
        options.push("original sent too".to_owned());
    }
    if outline.blind && outline.does != Does::Send {
        options.push("keeps modifiers".to_owned());
    }
    options
}

fn summary(outline: &RuleOutline) -> String {
    let caps = |caps: Vec<Cap>| {
        caps.into_iter()
            .map(|cap| cap.label)
            .collect::<Vec<_>>()
            .join(" + ")
    };
    let does = match outline.does {
        Does::Send => "acts as",
        Does::Tap => "taps",
        Does::Steps => "plays",
    };
    format!(
        "{} {does} {}",
        caps(trigger_caps(outline)),
        caps(result_caps(outline))
    )
}

#[component]
pub(super) fn RuleItem(edits: Edits, group: usize, rule: usize) -> NodeId {
    let outline = edits.rule(group, rule);
    let open = create_memo(move || edits.open.get() == Some((group, rule)));
    let on = create_memo(move || trigger_caps(&outline.get()));
    let result = create_memo(move || result_caps(&outline.get()));
    let taps = create_memo(move || outline.get().does == Does::Tap);
    let tags = create_memo(move || match open.get() {
        true => String::new(),
        false => options(&outline.get(), true).join(" \u{b7} "),
    });
    let label = create_memo(move || summary(&outline.get()));
    let toggle = move || {
        edits.open.update(|open| {
            *open = match *open == Some((group, rule)) {
                true => None,
                false => Some((group, rule)),
            }
        })
    };
    let first = rule == 0;
    view! {
        <List spacing=Space::None>
            <Show condition={!first}>
                <Frame padding={Padding::horizontal(Space::Xl)}>
                    <Separator />
                </Frame>
            </Show>
            <unstyled::Button
                label={label}
                expanded={open}
                @test_id={format!("input-remaps.group.{group}.rule.{rule}")}
                on_click={toggle}
            >|handle: unstyled::ButtonHandle|
                <RuleRowFace handle on result taps tags open />
            </unstyled::Button>
            <Show condition={open}>
                <RuleEditor edits={edits} group={group} rule={rule} outline={outline} />
            </Show>
        </List>
    }
}

#[component]
fn RuleRowFace(
    handle: unstyled::ButtonHandle,
    on: Memo<Vec<Cap>>,
    result: Memo<Vec<Cap>>,
    taps: Memo<bool>,
    tags: Memo<String>,
    open: Memo<bool>,
) -> NodeId {
    let fill = create_memo(move || match handle.hovered.get() {
        true => ColorRole::Hover,
        false => ColorRole::None,
    });
    let chevron = create_memo(move || match open.get() {
        true => ICON_CHEVRON_UP,
        false => ICON_CHEVRON_DOWN,
    });
    view! {
        <FocusRing focused={handle.focused} radius=Radius::Lg>
            <Frame fill={fill} radius=Radius::Lg padding=ROW_PADDING>
                <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
                    <Caps caps={on} />
                    <Text color=TextColor::Muted align=TextAlign::Center>
                        <Icon glyph=ICON_ARROW_RIGHT />
                    </Text>
                    <Show condition={taps}>
                        <Text style=TextStyle::CAPTION>"taps"</Text>
                    </Show>
                    <Caps caps={result} />
                    <Text
                        @sizing=ItemSize::Percent(100.0)
                        style=TextStyle::CAPTION
                        overflow=Overflow::Ellipsis
                        align=TextAlign::End
                    >
                        {tags}
                    </Text>
                    <Text color=TextColor::Muted align=TextAlign::Center>
                        <Icon glyph={chevron} />
                    </Text>
                </List>
            </Frame>
        </FocusRing>
    }
}

/// Key caps in a row, joined by `+`; muted ones (waits) are not joined.
#[component]
fn Caps(caps: Memo<Vec<Cap>>) -> NodeId {
    let indices = create_memo(move || (0..caps.with(Vec::len)).collect::<Vec<usize>>());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=CAP_SPACING wrap=true>
            <ForEach keys={indices}>|index: usize| {
                let cap = create_memo(move || caps.with(|caps| caps.get(index).cloned()));
                let text = create_memo(move || cap.get().map(|cap| cap.label).unwrap_or_default());
                let muted = create_memo(move || cap.get().is_some_and(|cap| cap.muted));
                let joined = create_memo(move || {
                    index > 0
                        && caps.with(|caps| {
                            caps.get(index).is_some_and(|cap| !cap.muted)
                                && caps.get(index - 1).is_some_and(|cap| !cap.muted)
                        })
                });
                view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=CAP_SPACING>
                        <Show condition={joined}>
                            <Text style=TextStyle::CAPTION>"+"</Text>
                        </Show>
                        <KeyCap label={text} muted={muted} />
                    </List>
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn RuleEditor(edits: Edits, group: usize, rule: usize, outline: Memo<RuleOutline>) -> NodeId {
    let id = move |part: &str| format!("input-remaps.group.{group}.rule.{rule}.{part}");
    let edit = move |edit: RuleEdit| edits.edit_rule(group, rule, edit);
    let read_only = edits.read_only;
    let trigger = create_memo(move || label(&Combo::parse(&outline.get().on).key));
    let held_modifiers = create_memo(move || {
        let outline = outline.get();
        held(&outline)
            .iter()
            .filter_map(|name| modifier_of(name))
            .collect::<Vec<_>>()
    });
    let held_keys = create_memo(move || {
        held(&outline.get())
            .into_iter()
            .filter(|name| modifier_of(name).is_none())
            .collect::<Vec<_>>()
    });
    let does = create_memo(move || match outline.get().does {
        Does::Send => 0,
        Does::Tap => 1,
        Does::Steps => 2,
    });
    let set_does = move |index: usize| {
        let chosen = match index {
            1 => Does::Tap,
            2 => Does::Steps,
            _ => Does::Send,
        };
        if chosen != outline.get_untracked().does {
            edit(RuleEdit::Does(chosen));
        }
    };
    let explained = create_memo(move || {
        match outline.get().does {
            Does::Send => "Held down for as long as you hold it.",
            Does::Tap => "Pressed and let go once, each time you press it.",
            Does::Steps => "Taps and waits, in order, each time you press it.",
        }
        .to_owned()
    });
    let keyed = create_memo(move || outline.get().does != Does::Steps);
    let tapping = create_memo(move || outline.get().does == Does::Tap);
    let stepping = create_memo(move || outline.get().does == Does::Steps);
    let not_send = create_memo(move || outline.get().does != Does::Send);
    let key_heading = create_memo(move || {
        match outline.get().does {
            Does::Tap => "Taps",
            _ => "Acts as",
        }
        .to_owned()
    });
    let key = create_memo(move || label(&Combo::parse(&outline.get().key).key));
    let key_modifiers = create_memo(move || Combo::parse(&outline.get().key).modifiers());
    let set_key_modifier = Callback::new(move |(modifier, on): (Modifier, bool)| {
        let mut chord = Combo::parse(&outline.get_untracked().key);
        match on {
            true => chord.hold(modifier.name()),
            false => chord.release(modifier.name()),
        }
        edit(RuleEdit::Key(chord.text()));
    });
    let hold = create_memo(move || outline.get().hold_ms.unwrap_or(0).max(0) as f64);
    let steps = create_memo(move || {
        (0..outline.with(|outline| outline.steps.len())).collect::<Vec<usize>>()
    });
    let not_held = create_memo(move || {
        outline
            .get()
            .not_held
            .iter()
            .filter_map(|name| modifier_of(name))
            .collect::<Vec<_>>()
    });
    let not_held_keys = create_memo(move || {
        outline
            .get()
            .not_held
            .into_iter()
            .filter(|name| modifier_of(name).is_none())
            .collect::<Vec<_>>()
    });
    let keep = create_memo(move || outline.get().keep);
    let blind = create_memo(move || outline.get().blind);
    let more = create_signal(false);
    let more_summary = create_memo(move || match more.get() {
        true => String::new(),
        false => options(&outline.get(), false).join(", "),
    });
    let kept = create_memo(move || format!("The program still gets {} as well.", trigger.get()));
    let add_tap = move || {
        let step = outline.get_untracked().steps.len();
        edit(RuleEdit::AddStep { wait: false });
        edits.pick(group, rule, Target::Step(step));
    };
    view! {
        <Frame padding=EDITOR_PADDING>
            <List spacing=PART_SPACING>
                <Part title="When you press">
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=CONTROL_SPACING
                        wrap=true
                    >
                        <KeyButton
                            label={trigger}
                            disabled={read_only}
                            id={id("on")}
                            on_click={move || edits.pick(group, rule, Target::On)}
                        />
                        <Text style=TextStyle::CAPTION>"while holding"</Text>
                        <ModifierToggles
                            on={held_modifiers}
                            disabled={read_only}
                            id={id("held")}
                            set={move |(modifier, on): (Modifier, bool)| {
                                edit(RuleEdit::Held(modifier.name().to_owned(), on))
                            }}
                        />
                        <RemovableKeys
                            names={held_keys}
                            disabled={read_only}
                            id={id("held-key")}
                            remove={move |name: String| edit(RuleEdit::Held(name, false))}
                        />
                        <Button
                            label="Another key…"
                            variant=ButtonVariant::Ghost
                            disabled={read_only}
                            @test_id={id("add-held")}
                            on_click={move || edits.pick(group, rule, Target::Held)}
                        />
                    </List>
                </Part>
                <Part title="It" description={explained}>
                    <SegmentedControl
                        options={view! {
                            <ChoiceOption label="Acts as another key" />
                            <ChoiceOption label="Taps a key" />
                            <ChoiceOption label="Plays a sequence" />
                        }}
                        selected={Binding::new(does, set_does)}
                        label="What it does"
                        @test_id={id("does")}
                    />
                </Part>
                <Show condition={keyed}>
                    <Part title={key_heading}>
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=CONTROL_SPACING
                            wrap=true
                        >
                            <ModifierToggles
                                on={key_modifiers}
                                disabled={read_only}
                                id={id("key")}
                                set={set_key_modifier}
                            />
                            <Text style=TextStyle::CAPTION>"+"</Text>
                            <KeyButton
                                label={key}
                                disabled={read_only}
                                id={id("key")}
                                on_click={move || edits.pick(group, rule, Target::Key)}
                            />
                        </List>
                    </Part>
                </Show>
                <Show condition={tapping}>
                    <Part
                        title="Hold the tap for"
                        description="Some games miss taps shorter than a frame or two."
                    >
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=CONTROL_SPACING
                            wrap=true
                        >
                            <HoldPresets
                                hold={hold}
                                disabled={read_only}
                                id={id("hold")}
                                set={move |ms: i64| edit(RuleEdit::HoldMs(Some(ms)))}
                            />
                            <Text style=TextStyle::CAPTION>"or"</Text>
                            <NumberInput
                                @sizing=ItemSize::Fixed(MS_WIDTH)
                                value={Binding::new(hold, move |ms: f64| edit(RuleEdit::HoldMs(Some(ms.round() as i64))))}
                                min=0.0
                                decimals={Some(0)}
                                label="Hold for, in milliseconds"
                                disabled={read_only}
                                @test_id={id("hold")}
                            />
                            <Text style=TextStyle::CAPTION>"ms"</Text>
                        </List>
                    </Part>
                </Show>
                <Show condition={stepping}>
                    <Part
                        title="Sequence"
                        description="Plays in order each time. Choose a key to change it."
                    >
                        <List spacing=CONTROL_SPACING>
                            <ForEach keys={steps}>|step: usize|
                                <StepTile
                                    edits={edits}
                                    group={group}
                                    rule={rule}
                                    step={step}
                                    outline={outline}
                                />
                            </ForEach>
                            <List direction=Direction::Horizontal spacing=CONTROL_SPACING>
                                <Button
                                    label="Tap a key"
                                    glyph={Some(ICON_PLUS)}
                                    size=ButtonSize::Small
                                    disabled={read_only}
                                    @test_id={id("add-tap")}
                                    on_click={add_tap}
                                />
                                <Button
                                    label="Wait"
                                    glyph={Some(ICON_PLUS)}
                                    size=ButtonSize::Small
                                    disabled={read_only}
                                    @test_id={id("add-wait")}
                                    on_click={move || edit(RuleEdit::AddStep { wait: true })}
                                />
                            </List>
                        </List>
                    </Part>
                </Show>
                <Accordion
                    title="More options"
                    detail={more_summary}
                    size=TextSize::Base
                    open={more}
                    @test_id={id("more")}
                >
                    <List spacing=PART_SPACING>
                        <MoreOption
                            title="Not while holding"
                            description="The remap steps aside while any of these are down."
                        >
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=CONTROL_SPACING
                                wrap=true
                            >
                                <ModifierToggles
                                    on={not_held}
                                    disabled={read_only}
                                    id={id("not-held")}
                                    set={move |(modifier, on): (Modifier, bool)| {
                                        edit(RuleEdit::NotHeld(modifier.name().to_owned(), on))
                                    }}
                                />
                                <RemovableKeys
                                    names={not_held_keys}
                                    disabled={read_only}
                                    id={id("not-held-key")}
                                    remove={move |name: String| edit(RuleEdit::NotHeld(name, false))}
                                />
                            </List>
                        </MoreOption>
                        <MoreOption title="Also send the original" description={kept}>
                            <Switch
                                on={Binding::new(keep, move |on: bool| edit(RuleEdit::Keep(on)))}
                                label="Also send the original"
                                disabled={read_only}
                                @test_id={id("keep")}
                            />
                        </MoreOption>
                        <Show condition={not_send}>
                            <MoreOption
                                title="Keep held modifiers"
                                description="Otherwise the Shift, Ctrl, Alt and Super you hold are let go during the taps."
                            >
                                <Switch
                                    on={Binding::new(blind, move |on: bool| edit(RuleEdit::Blind(on)))}
                                    label="Keep held modifiers"
                                    disabled={read_only}
                                    @test_id={id("blind")}
                                />
                            </MoreOption>
                        </Show>
                    </List>
                </Accordion>
                <List direction=Direction::Horizontal spacing=CONTROL_SPACING>
                    <Button
                        label="Duplicate"
                        glyph={Some(ICON_COPY)}
                        variant=ButtonVariant::Ghost
                        disabled={read_only}
                        @test_id={id("duplicate")}
                        on_click={move || edits.change.call(Change::DuplicateRule(group, rule))}
                    />
                    <Button
                        label="Delete remap"
                        glyph={Some(ICON_TRASH)}
                        variant=ButtonVariant::Ghost
                        disabled={read_only}
                        @test_id={id("remove")}
                        on_click={move || edits.change.call(Change::RemoveRule(group, rule))}
                    />
                </List>
            </List>
        </Frame>
    }
}

/// A part of the rule editor: a heading, what it means, and its controls below.
#[component]
fn Part(
    title: Prop<String>,
    #[prop(default = String::new())] description: Prop<String>,
    children: Child,
) -> NodeId {
    let title = create_memo(move || title.get());
    let description = create_memo(move || description.get());
    let described = create_memo(move || !description.get().is_empty());
    view! {
        <List spacing=CONTROL_SPACING>
            <List spacing=LABEL_SPACING>
                <Text weight=Weight::Bold>{title}</Text>
                <Show condition={described}>
                    <Text style=TextStyle::CAPTION>{description}</Text>
                </Show>
            </List>
            {children}
        </List>
    }
}

/// An option under "More options": what it is beside its control.
#[component]
fn MoreOption(title: &'static str, description: Prop<String>, children: Child) -> NodeId {
    let description = create_memo(move || description.get());
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <List @sizing=ItemSize::Percent(100.0) spacing=LABEL_SPACING>
                <Text>{title}</Text>
                <Text style=TextStyle::CAPTION>{description}</Text>
            </List>
            {children}
        </List>
    }
}

/// A key shown as its cap, which opens the picker to change it.
#[component]
fn KeyButton(
    label: Memo<String>,
    disabled: Memo<bool>,
    id: String,
    on_click: ClickCallback,
) -> NodeId {
    let named = create_memo(move || format!("Change {}", label.get()));
    view! {
        <unstyled::Button
            label={named}
            disabled={disabled}
            @test_id={id}
            on_click={on_click}
        >|handle: unstyled::ButtonHandle| {
            let fill = create_memo(move || match handle.hovered.get() && !handle.disabled.get() {
                true => ColorRole::Hover,
                false => ColorRole::None,
            });
            view! {
                <FocusRing focused={handle.focused}>
                    <Frame fill={fill} radius=Radius::Md padding={Padding::all(Space::Xs)}>
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=CONTROL_SPACING
                        >
                            <KeyCap label={label} />
                            <Show condition={create_memo(move || !handle.disabled.get())}>
                                <Text style={TextStyle::CAPTION.color(TextColor::Accent)}>
                                    "Change"
                                </Text>
                            </Show>
                        </List>
                    </Frame>
                </FocusRing>
            }
        }</unstyled::Button>
    }
}

/// Shift, Ctrl, Alt and Super as toggles.
#[component]
fn ModifierToggles(
    on: Memo<Vec<Modifier>>,
    disabled: Memo<bool>,
    id: String,
    set: Callback<(Modifier, bool)>,
) -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=CAP_SPACING>
            <ForEach keys={create_memo(|| Modifier::ALL.to_vec())}>|modifier: Modifier| {
                let pressed = create_memo(move || on.with(|on| on.contains(&modifier)));
                let control_id = format!("{id}.{}", modifier.name().to_lowercase());
                view! {
                    <ToggleButton
                        label={modifier.name()}
                        pressed={Binding::new(pressed, move |now: bool| set.call((modifier, now)))}
                        disabled={disabled}
                        @test_id={control_id}
                    />
                }
            }</ForEach>
        </List>
    }
}

/// Keys that are no modifier, each with a button that removes it.
#[component]
fn RemovableKeys(
    names: Memo<Vec<String>>,
    disabled: Memo<bool>,
    id: String,
    remove: Callback<String>,
) -> NodeId {
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=CAP_SPACING>
            <ForEach keys={names}>|name: String| {
                let shown = label(&name);
                let removed = name.clone();
                let control_id = format!("{id}.{}.remove", super::keys::id_part(&name));
                view! {
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::None>
                        <KeyCap label={shown.clone()} />
                        <Button
                            label={format!("Stop requiring {shown}")}
                            glyph={Some(ICON_X)}
                            icon_only=true
                            size=ButtonSize::Small
                            variant=ButtonVariant::Ghost
                            disabled={disabled}
                            @test_id={control_id}
                            on_click={move || remove.call(removed.clone())}
                        />
                    </List>
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn HoldPresets(hold: Memo<f64>, disabled: Memo<bool>, id: String, set: Callback<i64>) -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=CAP_SPACING>
            <ForEach
                keys={create_memo(|| HOLD_PRESETS.to_vec())}
            >|(ms, name): (i64, &'static str)| {
                let pressed = create_memo(move || hold.get().round() as i64 == ms);
                let control_id = format!("{id}.{ms}");
                view! {
                    <ToggleButton
                        label={name}
                        pressed={Binding::new(pressed, move |now: bool| {
                            if now {
                                set.call(ms);
                            }
                        })}
                        disabled={disabled}
                        @test_id={control_id}
                    />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn StepTile(
    edits: Edits,
    group: usize,
    rule: usize,
    step: usize,
    outline: Memo<RuleOutline>,
) -> NodeId {
    let id = move |part: &str| format!("input-remaps.group.{group}.rule.{rule}.step.{step}.{part}");
    let edit = move |edit: RuleEdit| edits.edit_rule(group, rule, edit);
    let shown = create_memo(move || {
        outline
            .with(|outline| outline.steps.get(step).cloned())
            .unwrap_or(StepOutline::Wait { ms: 0 })
    });
    let tap = create_memo(move || matches!(shown.get(), StepOutline::Tap { .. }));
    let wait = create_memo(move || !tap.get());
    let key = create_memo(move || match shown.get() {
        StepOutline::Tap { key, .. } => chord_caps(&key)
            .into_iter()
            .map(|cap| cap.label)
            .collect::<Vec<_>>()
            .join(" + "),
        StepOutline::Wait { .. } => String::new(),
    });
    let ms = create_memo(move || match shown.get() {
        StepOutline::Tap { hold_ms, .. } => hold_ms.unwrap_or(0) as f64,
        StepOutline::Wait { ms } => ms as f64,
    });
    let number = format!("{}", step + 1);
    view! {
        <Frame
            border={Border::DEFAULT}
            radius=Radius::Md
            padding={Padding::symmetric(Space::Md, Space::Xs)}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=CONTROL_SPACING>
                <Text style=TextStyle::CAPTION>{number}</Text>
                <Show condition={tap}>
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=CONTROL_SPACING
                    >
                        <KeyButton
                            label={key}
                            disabled={edits.read_only}
                            id={id("key")}
                            on_click={move || edits.pick(group, rule, Target::Step(step))}
                        />
                        <Text style=TextStyle::CAPTION>"held"</Text>
                    </List>
                </Show>
                <Show condition={wait}>
                    <Text style=TextStyle::CAPTION>
                        <Icon glyph=ICON_TIMER />
                        "wait"
                    </Text>
                </Show>
                <NumberInput
                    @sizing=ItemSize::Fixed(MS_WIDTH)
                    value={Binding::new(ms, move |ms: f64| edit(RuleEdit::StepMs(step, Some(ms.round() as i64))))}
                    min=0.0
                    decimals={Some(0)}
                    label="Milliseconds"
                    disabled={edits.read_only}
                    @test_id={id("ms")}
                />
                <Text @sizing=ItemSize::Percent(100.0) style=TextStyle::CAPTION>"ms"</Text>
                <Button
                    label={format!("Remove step {}", step + 1)}
                    glyph={Some(ICON_X)}
                    icon_only=true
                    size=ButtonSize::Small
                    variant=ButtonVariant::Ghost
                    disabled={edits.read_only}
                    @test_id={id("remove")}
                    on_click={move || edit(RuleEdit::RemoveStep(step))}
                />
            </List>
        </Frame>
    }
}
