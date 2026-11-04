use be_host_remap::edit::{
    Combo, Editing, GroupField, GroupOutline, RuleEdit, RuleOutline, StepOutline,
};
use be_host_remap::text;
use be_sdk_beui::Editor;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::icons::{ICON_PENCIL, ICON_PLUS, ICON_TRASH};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;

use super::picker::{KeyPicker, Picked, Picking, Target};
use super::programs::{ProgramDialog, Scope, Scoping};
use super::rule::RuleItem;
use super::ui::Shared;

const SECTION_SPACING: Space = Space::Xl3;
const EVERYWHERE: &str = "Everywhere";

#[derive(Clone, Debug)]
pub(super) enum Change {
    /// Adds a rule to a group, or with none, to a new group for every program.
    AddRule(Option<usize>),
    AddGroup(Scope),
    Scope(usize, Scope),
    RemoveGroup(usize),
    RemoveRule(usize, usize),
    DuplicateRule(usize, usize),
    Rule(usize, usize, RuleEdit),
}

/// What every part of the editor shares: the rules as written, the way to
/// change them, and what is open.
#[derive(Clone, Copy)]
pub(super) struct Edits {
    pub(super) outline: Memo<Vec<GroupOutline>>,
    pub(super) change: Callback<Change>,
    pub(super) read_only: Memo<bool>,
    /// The rule shown with its editor, by group and rule.
    pub(super) open: Signal<Option<(usize, usize)>>,
    pub(super) picking: Signal<Option<Picking>>,
    pub(super) scoping: Signal<Option<Scoping>>,
}

impl Edits {
    pub(super) fn group(self, group: usize) -> Memo<GroupOutline> {
        create_memo(move || {
            self.outline
                .with(|outline| outline.get(group).cloned().unwrap_or_default())
        })
    }

    pub(super) fn rule(self, group: usize, rule: usize) -> Memo<RuleOutline> {
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

    pub(super) fn edit_rule(self, group: usize, rule: usize, edit: RuleEdit) {
        self.change.call(Change::Rule(group, rule, edit));
    }

    pub(super) fn pick(self, group: usize, rule: usize, target: Target) {
        self.picking.set(Some(Picking {
            group,
            rule,
            target,
        }));
    }

    /// Writes a picked key where it was picked for.
    fn picked(self, picking: Picking, picked: Picked) {
        let Picking {
            group,
            rule,
            target,
        } = picking;
        let outline = self.outline.with_untracked(|outline| {
            outline
                .get(group)
                .and_then(|group| group.rules.get(rule))
                .cloned()
                .unwrap_or_default()
        });
        let combo = |written: &str| {
            let mut combo = Combo::parse(written);
            combo.key = picked.key.clone();
            if let Some(with) = &picked.with {
                combo.held = with.clone();
            }
            combo.text()
        };
        let edit = match target {
            Target::On if picked.with.is_some() => RuleEdit::On(combo(&outline.on)),
            Target::On => RuleEdit::Trigger(picked.key.clone()),
            Target::Held => RuleEdit::Held(picked.key.clone(), true),
            Target::Key => RuleEdit::Key(combo(&outline.key)),
            Target::Step(step) => {
                let written = match outline.steps.get(step) {
                    Some(StepOutline::Tap { key, .. }) => key.clone(),
                    _ => String::new(),
                };
                RuleEdit::StepKey(step, combo(&written))
            }
        };
        self.edit_rule(group, rule, edit);
    }
}

fn set_scope(editing: &mut Editing, group: usize, scope: &Scope) {
    editing.set_group(group, GroupField::Name, &scope.name);
    editing.set_group(group, GroupField::Exe, &scope.exe);
    editing.set_group(group, GroupField::ProgramId, &scope.program_id);
    editing.set_group(group, GroupField::Title, &scope.title);
}

fn everywhere(group: &GroupOutline) -> bool {
    group.exe.is_empty() && group.program_id.is_empty() && group.title.is_empty()
}

#[component]
pub(super) fn EditorTab(shared: Shared, editor: Editor) -> NodeId {
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
    let open = create_signal(None::<(usize, usize)>);
    let picking = create_signal(None::<Picking>);
    let scoping = create_signal(None::<Scoping>);
    let change = Callback::new(move |change: Change| {
        let Ok(mut editing) = Editing::parse(&draft.get_untracked()) else {
            return;
        };
        let mut then = None;
        match change {
            Change::AddRule(group) => {
                let group = group.unwrap_or_else(|| {
                    editing.add_group(EVERYWHERE);
                    editing.outline().len().saturating_sub(1)
                });
                editing.add_rule(group);
                let rule = editing
                    .outline()
                    .get(group)
                    .map_or(0, |group| group.rules.len().saturating_sub(1));
                open.set(Some((group, rule)));
                then = Some(Picking {
                    group,
                    rule,
                    target: Target::On,
                });
            }
            Change::AddGroup(scope) => {
                editing.add_group(&scope.name);
                let group = editing.outline().len().saturating_sub(1);
                set_scope(&mut editing, group, &scope);
            }
            Change::Scope(group, scope) => set_scope(&mut editing, group, &scope),
            Change::RemoveGroup(group) => {
                editing.remove_group(group);
                open.set(None);
            }
            Change::RemoveRule(group, rule) => {
                editing.remove_rule(group, rule);
                open.set(None);
            }
            Change::DuplicateRule(group, rule) => {
                editing.duplicate_rule(group, rule);
                open.set(Some((group, rule + 1)));
            }
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
        if then.is_some() {
            picking.set(then);
        }
    });
    let edits = Edits {
        outline,
        change,
        read_only: shared.read_only,
        open,
        picking,
        scoping,
    };
    let groups = create_memo(move || (0..outline.with(Vec::len)).collect::<Vec<usize>>());
    let blocked = create_memo(move || unreadable.get().is_some());
    let no_everywhere =
        create_memo(move || !blocked.get() && outline.with(|outline| !outline.iter().any(everywhere)));
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
    let current = Callback::new(move |group: usize| {
        let outline = outline.with_untracked(|outline| outline.get(group).cloned().unwrap_or_default());
        Scope {
            name: outline.name,
            exe: outline.exe,
            program_id: outline.program_id,
            title: outline.title,
        }
    });
    let apply = Callback::new(move |(scoping, scope): (Scoping, Scope)| match scoping {
        Scoping::Add => change.call(Change::AddGroup(scope)),
        Scoping::Group(group) => change.call(Change::Scope(group, scope)),
    });
    let choose = Callback::new(move |(picking, picked): (Picking, Picked)| edits.picked(picking, picked));
    view! {
        <List spacing=SECTION_SPACING>
            <Show condition={warned}>
                <SettingsSection>
                    <SettingsRow label="Problem">
                        <Text style={TextStyle::CAPTION.color(TextColor::Danger)}>{message}</Text>
                    </SettingsRow>
                </SettingsSection>
            </Show>
            <Show condition={no_everywhere}>
                <SettingsSection title=EVERYWHERE description="In every program.">
                    <SettingsRow
                        label="No remaps yet"
                        description="Add one, then pick the key you want to change."
                        compact=true
                    >
                        <Button
                            label="Add remap"
                            glyph={Some(ICON_PLUS)}
                            disabled={adding_off}
                            @test_id={"input-remaps.add-rule"}
                            on_click={move || change.call(Change::AddRule(None))}
                        />
                    </SettingsRow>
                </SettingsSection>
            </Show>
            <ForEach keys={groups}>|group: usize|
                <GroupSection edits={edits} group={group} />
            </ForEach>
            <SettingsSection>
                <SettingsRow
                    label="Programs"
                    description="Remaps that apply only while a program's window has the keyboard."
                    compact=true
                >
                    <Button
                        label="Add a program"
                        glyph={Some(ICON_PLUS)}
                        disabled={adding_off}
                        @test_id={"input-remaps.add-program"}
                        on_click={move || scoping.set(Some(Scoping::Add))}
                    />
                </SettingsRow>
            </SettingsSection>
            <KeyPicker editor={editor} picking={picking} choose={choose} />
            <ProgramDialog editor={editor} scoping={scoping} current={current} apply={apply} />
        </List>
    }
}

#[component]
fn GroupSection(edits: Edits, group: usize) -> NodeId {
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
            ("", outline.exe),
            ("program id ", outline.program_id),
            ("the title ", outline.title),
        ]
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(what, value)| format!("{what}{value}"))
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
    let empty = create_memo(move || rules.with(Vec::is_empty));
    view! {
        <SettingsSection title={title} description={scope}>
            <Show condition={empty}>
                <SettingsRow
                    label="No remaps here yet"
                    description="Add one, then pick the key you want to change."
                />
            </Show>
            <ForEach keys={rules}>|rule: usize|
                <RuleItem edits={edits} group={group} rule={rule} />
            </ForEach>
            <SettingsRow label="" separated=true compact=true>
                <Button
                    label="Add remap"
                    glyph={Some(ICON_PLUS)}
                    disabled={edits.read_only}
                    @test_id={format!("input-remaps.group.{group}.add-rule")}
                    on_click={move || edits.change.call(Change::AddRule(Some(group)))}
                />
                <Button
                    label="Where it applies"
                    glyph={Some(ICON_PENCIL)}
                    variant=ButtonVariant::Ghost
                    disabled={edits.read_only}
                    @test_id={format!("input-remaps.group.{group}.scope")}
                    on_click={move || edits.scoping.set(Some(Scoping::Group(group)))}
                />
                <Button
                    label="Remove group"
                    glyph={Some(ICON_TRASH)}
                    icon_only=true
                    variant=ButtonVariant::Ghost
                    disabled={edits.read_only}
                    @test_id={format!("input-remaps.group.{group}.remove")}
                    on_click={move || edits.change.call(Change::RemoveGroup(group))}
                />
            </SettingsRow>
        </SettingsSection>
    }
}
