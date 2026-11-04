//! The key picker: what a rule's key, held key, sent key or step is chosen
//! with, so no key name has to be typed. Typing one is the fallback at its foot.

use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{Align, Direction, ItemSize, NodeId};
use be_sdk_beui::beui::icons::ICON_SEARCH;

use super::keys::{self, id_part, label};

const SPACING: Space = Space::Lg;
const KEY_SPACING: Space = Space::Sm;
const CATEGORY_SPACING: Space = Space::Xs2;

/// What the picked key becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    /// What is pressed: a key, a button or a wheel turn.
    On,
    /// A key that must be held as well.
    Held,
    /// The key a rule acts as or taps.
    Key,
    /// The key a step taps.
    Step(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Picking {
    pub(super) group: usize,
    pub(super) rule: usize,
    pub(super) target: Target,
}

/// A picked key, and when it was pressed with others, what was held with it,
/// which replaces what the rule had held.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Picked {
    pub(super) key: String,
    pub(super) with: Option<Vec<String>>,
}

impl Picked {
    fn key(key: String) -> Self {
        Self { key, with: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Tab {
    Mouse,
    AllKeys,
}

impl Tab {
    const fn label(self) -> &'static str {
        match self {
            Self::Mouse => "Mouse",
            Self::AllKeys => "All keys",
        }
    }
}

#[component]
pub(super) fn KeyPicker(picking: Signal<Option<Picking>>, choose: Callback<(Picking, Picked)>) -> NodeId {
    let open = create_memo(move || picking.get().is_some());
    let target = create_memo(move || picking.get().map(|picking| picking.target));
    let wheel = create_memo(move || target.get() == Some(Target::On));
    let title = create_memo(move || {
        match target.get() {
            Some(Target::On) | None => "Choose what you press",
            Some(Target::Held) => "Choose a key to hold as well",
            Some(Target::Key) => "Choose the key it sends",
            Some(Target::Step(_)) => "Choose the key to tap",
        }
        .to_owned()
    });
    let tabs = create_memo(|| vec![Tab::Mouse, Tab::AllKeys]);
    let first = move || {
        tabs.with_untracked(|tabs| tabs.iter().position(|tab| *tab == Tab::AllKeys))
            .unwrap_or(0)
    };
    let tab = create_signal(first());
    let query = create_signal(String::new());
    let typing = create_signal(false);
    let typed = create_signal(String::new());
    create_effect(move || {
        if open.get() {
            untrack(|| {
                tab.set(first());
                query.set(String::new());
                typing.set(false);
                typed.set(String::new());
            });
        }
    });
    let shown = move |which: Tab| {
        create_memo(move || tabs.with(|tabs| tabs.get(tab.get()) == Some(&which)))
    };
    let pick = Callback::new(move |picked: Picked| {
        if let Some(now) = picking.get_untracked() {
            picking.set(None);
            choose.call((now, picked));
        }
    });
    view! {
        <Dialog
            open={open}
            title={title}
            id="input-remaps.picker"
            actions={vec![DialogAction::cancel("Cancel")]}
            on_dismiss={move || picking.set(None)}
        >
            <List spacing=SPACING>
                <Tabs
                    options={view! {
                        <ForEach keys={tabs}>|which: Tab|
                            <ChoiceOption label={which.label()} />
                        </ForEach>
                    }}
                    selected={tab}
                    label="Find the key by"
                    @test_id={"input-remaps.picker.tabs"}
                />
                <Show condition={shown(Tab::Mouse)}>
                    <MouseTab wheel={wheel} pick={pick} />
                </Show>
                <Show condition={shown(Tab::AllKeys)}>
                    <AllKeysTab query={query} wheel={wheel} pick={pick} />
                </Show>
                <Separator />
                <TypedKey open={typing} typed={typed} wheel={wheel} pick={pick} />
            </List>
        </Dialog>
    }
}

#[component]
fn MouseTab(wheel: Memo<bool>, pick: Callback<Picked>) -> NodeId {
    let buttons = create_memo(|| keys::buttons());
    let wheels = create_memo(|| keys::wheels());
    view! {
        <List spacing=SPACING>
            <KeyButtons title="Buttons" names={buttons} pick={pick} />
            <Show condition={wheel}>
                <KeyButtons title="Wheel" names={wheels} pick={pick} />
            </Show>
        </List>
    }
}

#[component]
fn AllKeysTab(query: Signal<String>, wheel: Memo<bool>, pick: Callback<Picked>) -> NodeId {
    let listed = create_memo(move || keys::listed(&query.get()));
    let wheels = create_memo(move || {
        let query = query.get().trim().to_lowercase();
        match wheel.get() && !query.is_empty() {
            true => keys::wheels()
                .into_iter()
                .filter(|name| {
                    name.to_lowercase().contains(&query) || label(name).to_lowercase().contains(&query)
                })
                .collect(),
            false => Vec::new(),
        }
    });
    let wheeled = create_memo(move || wheels.with(|wheels| !wheels.is_empty()));
    let kinds = create_memo(move || {
        listed.with(|listed| listed.iter().map(|(kind, _)| *kind).collect::<Vec<_>>())
    });
    let none = create_memo(move || kinds.with(Vec::is_empty) && !wheeled.get());
    let nothing = create_memo(move || format!("No key matches \u{201c}{}\u{201d}.", query.get().trim()));
    view! {
        <List spacing=SPACING>
            <TextInput
                value={query}
                placeholder="Search keys"
                label="Search keys"
                glyph={Some(ICON_SEARCH)}
                clearable=true
                @test_id={"input-remaps.picker.search"}
            />
            <ForEach keys={kinds}>|kind: be_host_remap::names::KeyKind| {
                let names = create_memo(move || {
                    listed.with(|listed| {
                        listed
                            .iter()
                            .find(|(listed, _)| *listed == kind)
                            .map(|(_, names)| names.clone())
                            .unwrap_or_default()
                    })
                });
                view! {
                    <KeyButtons title={kind.title()} names={names} pick={pick} />
                }
            }</ForEach>
            <Show condition={wheeled}>
                <KeyButtons title="Wheel" names={wheels} pick={pick} />
            </Show>
            <Show condition={none}>
                <Text style=TextStyle::CAPTION>{nothing}</Text>
            </Show>
        </List>
    }
}

#[component]
fn KeyButtons(title: Prop<String>, names: Memo<Vec<String>>, pick: Callback<Picked>) -> NodeId {
    let title = create_memo(move || title.get());
    view! {
        <List spacing=CATEGORY_SPACING>
            <Text style=TextStyle::CAPTION>{title}</Text>
            <List direction=Direction::Horizontal spacing=KEY_SPACING wrap=true>
                <ForEach keys={names}>|name: String| {
                    let chosen = name.clone();
                    view! {
                        <Button
                            label={label(&name)}
                            size=ButtonSize::Small
                            @test_id={format!("input-remaps.picker.key.{}", id_part(&name))}
                            on_click={move || pick.call(Picked::key(chosen.clone()))}
                        />
                    }
                }</ForEach>
            </List>
        </List>
    }
}

/// The fallback for a key no list has: its name, an alias or its code, read as
/// the TOML reads it.
#[component]
fn TypedKey(open: Signal<bool>, typed: Signal<String>, wheel: Memo<bool>, pick: Callback<Picked>) -> NodeId {
    let read = create_memo(move || keys::written(&typed.get(), wheel.get()));
    let empty = create_memo(move || typed.with(|typed| typed.trim().is_empty()));
    let unusable = create_memo(move || read.with(Result::is_err));
    let message = create_memo(move || match (empty.get(), read.get()) {
        (true, _) => "Any name the TOML reads, or a Linux key or button code.".to_owned(),
        (false, Ok(name)) => format!("Uses {}.", label(&name)),
        (false, Err(problem)) => problem,
    });
    let style = create_memo(move || match !empty.get() && unusable.get() {
        true => TextStyle::CAPTION.color(TextColor::Danger),
        false => TextStyle::CAPTION,
    });
    let use_it = move || {
        if let Ok(name) = read.get_untracked() {
            pick.call(Picked::key(name));
        }
    };
    view! {
        <Accordion
            title="Not listed? Enter a key name or code"
            size=TextSize::Base
            open={open}
            @test_id={"input-remaps.picker.typing"}
        >
            <List spacing=CATEGORY_SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=KEY_SPACING>
                    <TextInput
                        @sizing=ItemSize::Percent(100.0)
                        value={typed}
                        placeholder="F13, MouseTask, Key 0x9e"
                        label="Key name or code"
                        on_submit={move |_: String| use_it()}
                        @test_id={"input-remaps.picker.typed"}
                    />
                    <Button
                        label="Use"
                        variant=ButtonVariant::Primary
                        disabled={unusable}
                        @test_id={"input-remaps.picker.use"}
                        on_click={use_it}
                    />
                </List>
                <Text style={style}>{message}</Text>
            </List>
        </Accordion>
    }
}
