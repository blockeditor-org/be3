use be_remap::edit::{Does, Editing, GroupField, GroupOutline, RuleEdit, RuleOutline, StepOutline};
use be_remap::text;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{ItemSize, NodeId};

use super::ui::Shared;
use beui::Extent;

const ROW_SPACING: Space = Space::Md;
const MS_WIDTH: f32 = 96.0;

#[derive(Clone, Debug)]
enum Change {
    AddGroup,
    RemoveGroup(usize),
    Group(usize, GroupField, String),
    AddRule(usize),
    RemoveRule(usize, usize),
    Rule(usize, usize, RuleEdit),
}

#[derive(Clone, Copy)]
struct Edits {
    outline: Memo<Vec<GroupOutline>>,
    change: Callback<Change>,
    read_only: Memo<bool>,
}

impl Edits {
    fn group(self, group: usize) -> Memo<GroupOutline> {
        create_memo(move || {
            self.outline
                .with(|outline| outline.get(group).cloned().unwrap_or_default())
        })
    }

    fn rule(self, group: usize, rule: usize) -> Memo<RuleOutline> {
        create_memo(move || {
            self.outline.with(|outline| {
                outline
                    .get(group)
                    .and_then(|group| group.rules.get(rule))
                    .cloned()
                    .unwrap_or_default()
            })
        })
    }

    fn edit_rule(self, group: usize, rule: usize, edit: RuleEdit) {
        self.change.call(Change::Rule(group, rule, edit));
    }
}

#[component]
pub(super) fn EditorTab(shared: Shared) -> NodeId {
    let stored = shared.stored;
    let draft = create_signal(stored.get_untracked());
    let followed = store(std::cell::RefCell::new(stored.get_untracked()));
    create_effect(move || {
        let text = stored.get();
        untrack(|| {
            let current = draft.get();
            let following = followed.with(|followed| *followed.borrow() == current);
            if following && current != text {
                draft.set(text.clone());
            }
            followed.with(|followed| *followed.borrow_mut() = text);
        });
    });
    let parsed = create_memo(move || Editing::parse(&draft.get()).map(|editing| editing.outline()));
    let outline = create_memo(move || parsed.get().unwrap_or_default());
    let unreadable = create_memo(move || parsed.get().err());
    let problem = create_signal(None::<String>);
    let change = Callback::new(move |change: Change| {
        let Ok(mut editing) = Editing::parse(&draft.get_untracked()) else {
            return;
        };
        match change {
            Change::AddGroup => {
                let count = editing.outline().len();
                editing.add_group(&format!("Group {}", count + 1));
            }
            Change::RemoveGroup(group) => editing.remove_group(group),
            Change::Group(group, field, text) => editing.set_group(group, field, &text),
            Change::AddRule(group) => editing.add_rule(group),
            Change::RemoveRule(group, rule) => editing.remove_rule(group, rule),
            Change::Rule(group, rule, edit) => editing.edit_rule(group, rule, edit),
        }
        let text = editing.text();
        draft.set(text.clone());
        match text::parse(&text) {
            Ok(_) => {
                problem.set(None);
                followed.with(|followed| *followed.borrow_mut() = text.clone());
                shared.store(&text);
            }
            Err(error) => problem.set(Some(error)),
        }
    });
    let edits = Edits {
        outline,
        change,
        read_only: shared.read_only,
    };
    let groups = create_memo(move || (0..outline.with(Vec::len)).collect::<Vec<usize>>());
    let empty = create_memo(move || groups.with(Vec::is_empty) && unreadable.get().is_none());
    let blocked = create_memo(move || unreadable.get().is_some());
    let message = create_memo(move || {
        unreadable
            .get()
            .map(|error| {
                format!("The TOML does not read, so it can only be fixed as text: {error}")
            })
            .or_else(|| {
                problem
                    .get()
                    .map(|problem| format!("Not stored yet: {problem}"))
            })
            .unwrap_or_default()
    });
    let warned = create_memo(move || !message.get().is_empty());
    let adding_off = create_memo(move || shared.read_only.get() || blocked.get());
    view! {
        <List spacing=ROW_SPACING>
            <Show condition={warned}>
                <SettingsSection>
                    <SettingsRow label="Problem">
                        <Text style={TextStyle::CAPTION.color(TextColor::Danger)}>{message}</Text>
                    </SettingsRow>
                </SettingsSection>
            </Show>
            <ForEach keys={groups}>|group: usize|
                <GroupCard edits={edits} group={group} />
            </ForEach>
            <SettingsSection>
                <Show condition={empty}>
                    <SettingsRow
                        label="No remaps yet"
                        description="A group holds rules for every program, or only for the programs it names."
                    />
                </Show>
                <SettingsRow
                    label="Groups"
                    separated={create_memo(move || empty.get())}
                    compact=true
                >
                    <Button
                        label="Add group"
                        variant=ButtonVariant::Secondary
                        disabled={adding_off}
                        @test_id={"input-remaps.add-group"}
                        on_click={move || change.call(Change::AddGroup)}
                    />
                </SettingsRow>
            </SettingsSection>
        </List>
    }
}

#[component]
fn GroupCard(edits: Edits, group: usize) -> NodeId {
    let outline = edits.group(group);
    let title = create_memo(move || {
        let name = outline.get().name;
        match name.is_empty() {
            true => format!("Group {}", group + 1),
            false => name,
        }
    });
    let scope = create_memo(move || {
        let outline = outline.get();
        let named: Vec<String> = [
            ("program", outline.exe),
            ("app id", outline.app_id),
            ("title", outline.title),
        ]
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(what, value)| format!("{what} {value}"))
        .collect();
        match named.is_empty() {
            true => "In every program.".to_owned(),
            false => format!(
                "While the window with the keyboard matches {}.",
                named.join(", ")
            ),
        }
    });
    let rules = create_memo(move || {
        (0..outline.with(|outline| outline.rules.len())).collect::<Vec<usize>>()
    });
    let field = move |field: GroupField| {
        create_memo(move || {
            let outline = outline.get();
            match field {
                GroupField::Name => outline.name,
                GroupField::Exe => outline.exe,
                GroupField::AppId => outline.app_id,
                GroupField::Title => outline.title,
            }
        })
    };
    let set = move |field: GroupField| {
        Callback::new(move |text: String| edits.change.call(Change::Group(group, field, text)))
    };
    view! {
        <SettingsSection title={title} description={scope}>
            <TextRow
                label="Name"
                description=""
                value={field(GroupField::Name)}
                placeholder=""
                read_only={edits.read_only}
                id={format!("input-remaps.group.{group}.name")}
                separated=false
                on_change={set(GroupField::Name)}
            />
            <TextRow
                label="Program"
                description="The window's executable, like Celeste.exe; * matches anything."
                value={field(GroupField::Exe)}
                placeholder="Any"
                read_only={edits.read_only}
                id={format!("input-remaps.group.{group}.exe")}
                on_change={set(GroupField::Exe)}
            />
            <TextRow
                label="App id"
                description=""
                value={field(GroupField::AppId)}
                placeholder="Any"
                read_only={edits.read_only}
                id={format!("input-remaps.group.{group}.app-id")}
                on_change={set(GroupField::AppId)}
            />
            <TextRow
                label="Window title"
                description=""
                value={field(GroupField::Title)}
                placeholder="Any"
                read_only={edits.read_only}
                id={format!("input-remaps.group.{group}.title")}
                on_change={set(GroupField::Title)}
            />
            <ForEach keys={rules}>|rule: usize|
                <RuleRows edits={edits} group={group} rule={rule} />
            </ForEach>
            <SettingsRow label="" separated=true compact=true>
                <Button
                    label="Add rule"
                    variant=ButtonVariant::Secondary
                    disabled={edits.read_only}
                    @test_id={format!("input-remaps.group.{group}.add-rule")}
                    on_click={move || edits.change.call(Change::AddRule(group))}
                />
                <Button
                    label="Remove group"
                    variant=ButtonVariant::Ghost
                    disabled={edits.read_only}
                    @test_id={format!("input-remaps.group.{group}.remove")}
                    on_click={move || edits.change.call(Change::RemoveGroup(group))}
                />
            </SettingsRow>
        </SettingsSection>
    }
}

#[component]
fn RuleRows(edits: Edits, group: usize, rule: usize) -> NodeId {
    let outline = edits.rule(group, rule);
    let id = move |part: &str| format!("input-remaps.group.{group}.rule.{rule}.{part}");
    let summary = create_memo(move || {
        let outline = outline.get();
        let does = match outline.does {
            Does::Send => format!("holds {}", outline.key),
            Does::Tap => format!("taps {}", outline.key),
            Does::Steps => format!("plays {} steps", outline.steps.len()),
        };
        format!("{} {does}", outline.on)
    });
    let on = create_memo(move || outline.get().on);
    let key = create_memo(move || outline.get().key);
    let not_held = create_memo(move || outline.get().not_held);
    let does = create_memo(move || {
        Some(match outline.get().does {
            Does::Send => 0,
            Does::Tap => 1,
            Does::Steps => 2,
        })
    });
    let keyed = create_memo(move || outline.get().does != Does::Steps);
    let tapping = create_memo(move || outline.get().does == Does::Tap);
    let stepping = create_memo(move || outline.get().does == Does::Steps);
    let sends = create_memo(move || outline.get().does != Does::Send);
    let hold = create_memo(move || outline.get().hold_ms.unwrap_or(0) as f64);
    let keep = create_memo(move || outline.get().keep);
    let blind = create_memo(move || outline.get().blind);
    let steps = create_memo(move || {
        (0..outline.with(|outline| outline.steps.len())).collect::<Vec<usize>>()
    });
    let edit = move |change: fn(String) -> RuleEdit| {
        Callback::new(move |text: String| edits.edit_rule(group, rule, change(text)))
    };
    view! {
        <List spacing=Space::None>
            <SettingsRow
                label={format!("Rule {}", rule + 1)}
                description={summary}
                separated=true
                compact=true
            >
                <Button
                    label="Remove"
                    variant=ButtonVariant::Ghost
                    disabled={edits.read_only}
                    @test_id={id("remove")}
                    on_click={move || edits.change.call(Change::RemoveRule(group, rule))}
                />
            </SettingsRow>
            <TextRow
                label="When"
                description="A key, button or wheel turn, after any held with it: F20+WheelUp."
                value={on}
                placeholder=""
                read_only={edits.read_only}
                id={id("on")}
                on_change={edit(RuleEdit::On)}
            />
            <SettingsRow label="Does" separated=true>
                <Select
                    @sizing=ItemSize::Percent(100.0)
                    width={Extent::fraction(1.0)}
                    options={view! {
                        <ChoiceOption label="Holds a key while it is held" />
                        <ChoiceOption label="Taps a key" />
                        <ChoiceOption label="Plays steps" />
                    }}
                    selected={Binding::new(does, move |index: Option<usize>| {
                        let chosen = match index {
                            Some(1) => Does::Tap,
                            Some(2) => Does::Steps,
                            _ => Does::Send,
                        };
                        edits.edit_rule(group, rule, RuleEdit::Does(chosen));
                    })}
                    label="Does"
                    disabled={edits.read_only}
                    @test_id={id("does")}
                />
            </SettingsRow>
            <Show condition={keyed}>
                <TextRow
                    label="Key"
                    description="With any held with it: Shift+W."
                    value={key}
                    placeholder=""
                    read_only={edits.read_only}
                    id={id("key")}
                    on_change={edit(RuleEdit::Key)}
                />
            </Show>
            <Show condition={tapping}>
                <SettingsRow
                    label="Hold for"
                    description="Milliseconds, for games that miss short taps."
                    separated=true
                >
                    <NumberInput
                        @sizing=ItemSize::Fixed(MS_WIDTH)
                        value={Binding::new(hold, move |ms: f64| edits.edit_rule(group, rule, RuleEdit::HoldMs(Some(ms.round() as i64))))}
                        min=0.0
                        decimals={Some(0)}
                        label="Hold for"
                        disabled={edits.read_only}
                        @test_id={id("hold")}
                    />
                </SettingsRow>
            </Show>
            <Show condition={stepping}>
                <List spacing=Space::None>
                    <ForEach keys={steps}>|step: usize|
                        <StepRow edits={edits} group={group} rule={rule} step={step} />
                    </ForEach>
                    <SettingsRow label="" separated=true compact=true>
                        <Button
                            label="Add tap"
                            variant=ButtonVariant::Secondary
                            disabled={edits.read_only}
                            @test_id={id("add-tap")}
                            on_click={move || edits.edit_rule(group, rule, RuleEdit::AddStep { wait: false })}
                        />
                        <Button
                            label="Add wait"
                            variant=ButtonVariant::Secondary
                            disabled={edits.read_only}
                            @test_id={id("add-wait")}
                            on_click={move || edits.edit_rule(group, rule, RuleEdit::AddStep { wait: true })}
                        />
                    </SettingsRow>
                </List>
            </Show>
            <TextRow
                label="Unless held"
                description="Keys that keep the rule from applying, like Shift."
                value={not_held}
                placeholder="Nothing"
                read_only={edits.read_only}
                id={id("not-held")}
                on_change={edit(RuleEdit::NotHeld)}
            />
            <SettingsRow
                label="Passes it on too"
                description="The program still gets what was pressed."
                separated=true
                compact=true
            >
                <Switch
                    on={Binding::new(keep, move |on: bool| edits.edit_rule(group, rule, RuleEdit::Keep(on)))}
                    label="Passes it on too"
                    disabled={edits.read_only}
                    @test_id={id("keep")}
                />
            </SettingsRow>
            <Show condition={sends}>
                <SettingsRow
                    label="Keeps held modifiers"
                    description="Otherwise held Shift, Ctrl, Alt and Super are let go during the taps."
                    separated=true
                    compact=true
                >
                    <Switch
                        on={Binding::new(blind, move |on: bool| edits.edit_rule(group, rule, RuleEdit::Blind(on)))}
                        label="Keeps held modifiers"
                        disabled={edits.read_only}
                        @test_id={id("blind")}
                    />
                </SettingsRow>
            </Show>
        </List>
    }
}

#[component]
fn StepRow(edits: Edits, group: usize, rule: usize, step: usize) -> NodeId {
    let outline = create_memo(move || {
        edits.outline.with(|outline| {
            outline
                .get(group)
                .and_then(|group| group.rules.get(rule))
                .and_then(|rule| rule.steps.get(step))
                .cloned()
                .unwrap_or(StepOutline::Wait { ms: 0 })
        })
    });
    let id = move |part: &str| format!("input-remaps.group.{group}.rule.{rule}.step.{step}.{part}");
    let tap = create_memo(move || matches!(outline.get(), StepOutline::Tap { .. }));
    let key = create_memo(move || match outline.get() {
        StepOutline::Tap { key, .. } => key,
        StepOutline::Wait { .. } => String::new(),
    });
    let ms = create_memo(move || match outline.get() {
        StepOutline::Tap { hold_ms, .. } => hold_ms.unwrap_or(0) as f64,
        StepOutline::Wait { ms } => ms as f64,
    });
    let label = create_memo(move || match tap.get() {
        true => format!("Step {}: tap", step + 1),
        false => format!("Step {}: wait", step + 1),
    });
    view! {
        <SettingsRow label={label} separated=true>
            <Show condition={tap}>
                <TextInput
                    @sizing=ItemSize::Percent(100.0)
                    value={Binding::new(key, move |text: String| edits.edit_rule(group, rule, RuleEdit::StepKey(step, text)))}
                    label="Key"
                    disabled={edits.read_only}
                    @test_id={id("key")}
                />
            </Show>
            <NumberInput
                @sizing=ItemSize::Fixed(MS_WIDTH)
                value={Binding::new(ms, move |ms: f64| edits.edit_rule(group, rule, RuleEdit::StepMs(step, Some(ms.round() as i64))))}
                min=0.0
                decimals={Some(0)}
                label="Milliseconds"
                disabled={edits.read_only}
                @test_id={id("ms")}
            />
            <Button
                label="Remove"
                variant=ButtonVariant::Ghost
                disabled={edits.read_only}
                @test_id={id("remove")}
                on_click={move || edits.edit_rule(group, rule, RuleEdit::RemoveStep(step))}
            />
        </SettingsRow>
    }
}

#[component]
fn TextRow(
    label: &'static str,
    description: &'static str,
    value: Memo<String>,
    placeholder: &'static str,
    read_only: Memo<bool>,
    id: String,
    #[prop(default = true)] separated: bool,
    on_change: Callback<String>,
) -> NodeId {
    view! {
        <SettingsRow label={label} description={description} separated={separated}>
            <TextInput
                @sizing=ItemSize::Percent(100.0)
                value={Binding::new(value, move |text: String| {
                    if text != value.get_untracked() {
                        on_change.call(text);
                    }
                })}
                label={label}
                placeholder={placeholder}
                disabled={read_only}
                @test_id={id}
            />
        </SettingsRow>
    }
}
