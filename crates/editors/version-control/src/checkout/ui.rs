use beui::Overflow;
use block_editor_beui::be_block::Root as _;
use block_editor_beui::be_block::{CheckoutContent, ConflictKind, Repository};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_ARROW_DOWN_TO_LINE, ICON_CHECK, ICON_GIT_BRANCH, ICON_GIT_MERGE,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{
    BlockLink, ChildTarget, ConflictSide, Editor, VersionChangeKind, VersionCommand,
    VersionHistory, VersionSteps, short_id,
};
use uuid::Uuid;

const PADDING: f32 = 24.0;
const SECTION_SPACING: f32 = 18.0;

type ChangeRow = (Uuid, Uuid, Option<String>, &'static str);

type ConflictRow = (
    Uuid,
    Uuid,
    &'static str,
    Option<Uuid>,
    Option<Uuid>,
    Option<Uuid>,
);

fn change_label(kind: VersionChangeKind) -> &'static str {
    match kind {
        VersionChangeKind::Added => "Added",
        VersionChangeKind::Removed => "Removed",
        VersionChangeKind::Modified => "Changed",
        VersionChangeKind::Moved => "Moved or renamed",
    }
}

fn conflict_label(kind: ConflictKind) -> &'static str {
    match kind {
        ConflictKind::Content => "Changed here and in the incoming changes",
        ConflictKind::Placement => "Moved to different places",
        ConflictKind::Name => "Given different names",
        ConflictKind::EditedAndRemoved => "Changed on one side and removed on the other",
        ConflictKind::Metadata => "Its details were changed both ways",
    }
}

#[component]
pub fn CheckoutView(editor: Editor) -> NodeId {
    let content = editor.block_content::<CheckoutContent>();
    let state = content.project(|content| content.root());
    let status = editor.version_status();
    let editable = editor.editable();
    let summary = create_memo(move || {
        let branch = state.with(|state| state.branch.clone());
        status.with(|status| match &status.working {
            Some(working) => format!(
                "Editing on {branch}, in change {}",
                short_id(&working.change)
            ),
            None => format!("Editing on {branch}"),
        })
    });
    let repository = create_memo(move || {
        state.with(|state| {
            state
                .repository
                .map(|repository| ChildTarget::new(repository, Repository::CONTENT_TYPE))
        })
    });
    let idle = create_memo(move || editable.get() && !status.with(|status| status.busy));
    let blocked = create_memo(move || !idle.get());
    let note = create_memo(move || {
        status.with(|status| match (status.busy, &status.error) {
            (true, _) => "Working…".to_owned(),
            (false, Some(error)) => error.clone(),
            (false, None) => String::new(),
        })
    });
    let has_note = create_memo(move || !note.with(String::is_empty));
    let behind = create_memo(move || status.with(|status| status.behind));
    let changes = create_memo(move || {
        status.with(|status| {
            status
                .changes
                .iter()
                .map(|change| -> ChangeRow {
                    (
                        Uuid::from_bytes(change.block_id),
                        Uuid::from_bytes(change.block_type),
                        change.name.clone(),
                        change_label(change.kind),
                    )
                })
                .collect::<Vec<_>>()
        })
    });
    let unchanged = create_memo(move || changes.with(Vec::is_empty));
    let conflicts = create_memo(move || {
        state.with(|state| {
            state
                .conflicts
                .iter()
                .map(|conflict| -> ConflictRow {
                    (
                        conflict.block,
                        conflict.content_type,
                        conflict_label(conflict.kind),
                        conflict.base,
                        conflict.ours,
                        conflict.theirs,
                    )
                })
                .collect::<Vec<_>>()
        })
    });
    let conflicted = create_memo(move || !conflicts.with(Vec::is_empty));
    let combining = create_memo(move || {
        status.with(|status| {
            status
                .working
                .as_ref()
                .is_some_and(|working| working.parents.len() > 1)
        })
    });
    let combined_note = create_memo(move || combining.get() && unchanged.get());
    let empty_note = create_memo(move || !combining.get() && unchanged.get());
    let branches = create_memo(move || {
        let current = state.with(|state| state.branch.clone());
        status.with(|status| {
            status
                .branches
                .iter()
                .map(|branch| (branch.name.clone(), branch.name == current))
                .collect::<Vec<_>>()
        })
    });
    let log = create_memo(move || status.with(|status| status.log.clone()));
    let operations = create_memo(move || status.with(|status| status.operations.clone()));

    let description = create_signal(String::new());
    let save_blocked = create_memo(move || {
        blocked.get()
            || (unchanged.get() && !combining.get())
            || description.with(|description| description.trim().is_empty())
    });
    let branch_name = create_signal(String::new());
    let branch_blocked =
        create_memo(move || blocked.get() || branch_name.with(|name| name.trim().is_empty()));

    let change_rows = move |row: ChangeRow| {
        view! {
            <ChangeLine editor row />
        }
    };
    let conflict_rows = move |row: ConflictRow| {
        let blocked = blocked;
        view! {
            <ConflictLine editor blocked row />
        }
    };
    let branch_rows = move |(name, current): (String, bool)| {
        let blocked = blocked;
        view! {
            <BranchLine editor blocked name current />
        }
    };

    view! {
        <List spacing=0.0>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Frame padding=PADDING>
                    <List spacing=SECTION_SPACING>
                        <List spacing=6.0>
                            <Text style=TextStyle::HEADING>"Checkout"</Text>
                            <Text style=TextStyle::CAPTION @test_id={"checkout.summary"}>
                                {summary}
                            </Text>
                            <BlockLink
                                editor={editor}
                                block={repository}
                                @test_id={"checkout.repository"}
                            />
                            <Show condition={has_note}>
                                <Text style=TextStyle::CAPTION @test_id={"checkout.note"}>
                                    {note}
                                </Text>
                            </Show>
                        </List>
                        <Show condition={behind}>
                            <Card>
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=10.0
                                >
                                    <Text @sizing=ItemSize::Percent(100.0)>
                                        "Someone saved newer changes on this branch."
                                    </Text>
                                    <Button
                                        label="Bring them in"
                                        glyph=ICON_ARROW_DOWN_TO_LINE
                                        variant=ButtonVariant::Primary
                                        disabled={blocked}
                                        @test_id={"checkout.update"}
                                        on_click={move || editor.version(VersionCommand::Update)}
                                    />
                                </List>
                            </Card>
                        </Show>
                        <Show condition={conflicted}>
                            <Card>
                                <List spacing=12.0>
                                    <Text style=TextStyle::HEADING>"Clashing edits"</Text>
                                    <Text style=TextStyle::CAPTION>
                                        "These blocks were changed in two ways at once. Choose a version for each, or edit it and keep it as it is. You can keep working and save changes before you decide."
                                    </Text>
                                    <ForEach keys={conflicts} view={conflict_rows} />
                                </List>
                            </Card>
                        </Show>
                        <Card>
                            <List spacing=10.0>
                                <Text style=TextStyle::HEADING>"Current change"</Text>
                                <Text style=TextStyle::CAPTION>
                                    "Your edits are kept here as you make them. Describe them and save to start a new change on top."
                                </Text>
                                <Show condition={empty_note}>
                                    <Text style=TextStyle::CAPTION>
                                        "Nothing has changed since the last saved change."
                                    </Text>
                                </Show>
                                <Show condition={combined_note}>
                                    <Text style=TextStyle::CAPTION>
                                        "This change combines another branch into this one."
                                    </Text>
                                </Show>
                                <ForEach keys={changes} view={change_rows} />
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=10.0
                                >
                                    <TextInput
                                        @sizing=ItemSize::Percent(100.0)
                                        value={description}
                                        placeholder="What does this change do?"
                                        label="Description"
                                        @test_id={"checkout.description"}
                                    />
                                    <Button
                                        label="Save change"
                                        glyph=ICON_CHECK
                                        variant=ButtonVariant::Primary
                                        disabled={save_blocked}
                                        @test_id={"checkout.save"}
                                        on_click={move || {
                                            let text = description.get().trim().to_owned();
                                            editor.version(VersionCommand::Finish { description: text });
                                            description.set(String::new());
                                        }}
                                    />
                                </List>
                            </List>
                        </Card>
                        <Card>
                            <List spacing=10.0>
                                <Text style=TextStyle::HEADING>"Branches"</Text>
                                <ForEach keys={branches} view={branch_rows} />
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=10.0
                                >
                                    <TextInput
                                        @sizing=ItemSize::Percent(100.0)
                                        value={branch_name}
                                        placeholder="New branch"
                                        label="New branch name"
                                        @test_id={"checkout.branch-name"}
                                    />
                                    <Button
                                        label="Make branch"
                                        glyph=ICON_GIT_BRANCH
                                        variant=ButtonVariant::Secondary
                                        disabled={branch_blocked}
                                        @test_id={"checkout.make-branch"}
                                        on_click={move || {
                                            let name = branch_name.get().trim().to_owned();
                                            editor.version(VersionCommand::CreateBranch { name });
                                            branch_name.set(String::new());
                                        }}
                                    />
                                </List>
                            </List>
                        </Card>
                        <Card>
                            <List spacing=10.0>
                                <Text style=TextStyle::HEADING>"Saved changes"</Text>
                                <VersionHistory log={log} />
                            </List>
                        </Card>
                        <Card>
                            <List spacing=10.0>
                                <Text style=TextStyle::HEADING>"What was done"</Text>
                                <Text style=TextStyle::CAPTION>
                                    "Every step is kept here, so any of them can be undone."
                                </Text>
                                <VersionSteps
                                    editor={editor}
                                    operations={operations}
                                    blocked={blocked}
                                />
                            </List>
                        </Card>
                    </List>
                </Frame>
            </Scroll>
        </List>
    }
}

#[component]
fn ChangeLine(editor: Editor, row: ChangeRow) -> NodeId {
    let (block, block_type, name, label) = row;
    let removed = label == change_label(VersionChangeKind::Removed);
    let name = name.unwrap_or_else(|| "Untitled".to_owned());
    let target = Some(ChildTarget::new(block, block_type));
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=10.0
            @test_id={format!("checkout.change.{block}")}
        >
            <Text style=TextStyle::CAPTION>{label}</Text>
            <Show condition={removed}>|| {
                let name = name.clone(); view! {
                    <Text>{name}</Text>
                }
            }</Show>
            <Show condition={!removed}>
                <BlockLink editor={editor} block={target} />
            </Show>
        </List>
    }
}

#[component]
fn ConflictLine(editor: Editor, blocked: Memo<bool>, row: ConflictRow) -> NodeId {
    let (block, block_type, label, base, ours, theirs) = row;
    let side = |side: Option<Uuid>| side.map(|side| ChildTarget::new(side, block_type));
    let choose = |take: ConflictSide| {
        move || {
            editor.version(VersionCommand::Resolve {
                block_id: block.into_bytes(),
                take,
            });
        }
    };
    let compared = base.is_some() || ours.is_some() || theirs.is_some();
    let (take_base, take_ours, take_theirs) = (
        choose(ConflictSide::Base),
        choose(ConflictSide::Ours),
        choose(ConflictSide::Theirs),
    );
    let (base, ours, theirs) = (side(base), side(ours), side(theirs));
    let keep = choose(ConflictSide::Merged);
    let working = Some(ChildTarget::new(block, block_type));
    view! {
        <List spacing=8.0 @test_id={format!("checkout.conflict.{block}")}>
            <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                <BlockLink editor={editor} block={working} />
                <Text style=TextStyle::CAPTION>{label}</Text>
            </List>
            <Show condition={compared}>
                <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                    <Text style=TextStyle::CAPTION>"Before"</Text>
                    <BlockLink editor={editor} block={base} fallback="none" />
                    <Text style=TextStyle::CAPTION>"Yours"</Text>
                    <BlockLink editor={editor} block={ours} fallback="none" />
                    <Text style=TextStyle::CAPTION>"Incoming"</Text>
                    <BlockLink editor={editor} block={theirs} fallback="none" />
                </List>
            </Show>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Button
                    label="Use before"
                    variant=ButtonVariant::Secondary
                    disabled={blocked}
                    @test_id={format!("checkout.base.{block}")}
                    on_click={take_base}
                />
                <Button
                    label="Use yours"
                    variant=ButtonVariant::Secondary
                    disabled={blocked}
                    @test_id={format!("checkout.ours.{block}")}
                    on_click={take_ours}
                />
                <Button
                    label="Use incoming"
                    variant=ButtonVariant::Secondary
                    disabled={blocked}
                    @test_id={format!("checkout.theirs.{block}")}
                    on_click={take_theirs}
                />
                <Button
                    label="Keep as it is"
                    variant=ButtonVariant::Secondary
                    disabled={blocked}
                    @test_id={format!("checkout.keep.{block}")}
                    on_click={keep}
                />
            </List>
        </List>
    }
}

#[component]
fn BranchLine(editor: Editor, blocked: Memo<bool>, name: String, current: bool) -> NodeId {
    let (switched, merged) = (name.clone(), name.clone());
    let disabled = create_memo(move || blocked.get() || current);
    let label = match current {
        true => "You are here",
        false => "Move here",
    };
    let branch_test_id = format!("checkout.branch.{name}");
    let switch_test_id = format!("checkout.switch.{name}");
    let merge_test_id = format!("checkout.merge.{name}");
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=10.0
            @test_id=branch_test_id
        >
            <Text overflow=Overflow::Ellipsis @sizing=ItemSize::Percent(100.0)>{name}</Text>
            <Show condition={!current}>|| {
                let (merged, merge_test_id) = (merged.clone(), merge_test_id.clone());
                view! {
                    <Button
                        label="Combine into this"
                        glyph=ICON_GIT_MERGE
                        variant=ButtonVariant::Ghost
                        disabled={blocked}
                        @test_id=merge_test_id
                        on_click={move || editor.version(VersionCommand::Merge { branch: merged.clone() })}
                    />
                }
            }</Show>
            <Button
                label={label}
                variant=ButtonVariant::Secondary
                disabled={disabled}
                @test_id=switch_test_id
                on_click={move || editor.version(VersionCommand::Switch { branch: switched.clone() })}
            />
        </List>
    }
}
