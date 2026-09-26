use block_editor_beui::be_block::Root as _;
use block_editor_beui::be_block::{CheckoutContent, ConflictKind, Repository};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, clone, component, create_memo,
    create_signal, view,
};
use block_editor_beui::beui::styled::{
    Body, Button, ButtonVariant, Caption, Card, Heading, Scroll, TextInput, use_theme,
};
use block_editor_beui::{
    BlockLink, ChildTarget, ConflictSide, Editor, VersionChangeKind, VersionCommand,
    VersionHistory, short_id,
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
        ConflictKind::Content => "Both sides changed it",
        ConflictKind::Placement => "Both sides moved it",
        ConflictKind::Name => "Both sides renamed it",
        ConflictKind::EditedAndRemoved => "Changed on one side and removed on the other",
    }
}

#[component]
pub fn CheckoutView(editor: Editor) -> NodeId {
    let content = editor.block_content::<CheckoutContent>();
    let state = content.project(|content| content.root());
    let status = editor.version_status();
    let editable = editor.editable();
    let summary = create_memo(clone!(state -> move || {
        state.with(|state| match state.base {
            Some(base) => format!(
                "On {}, at {}",
                state.branch,
                short_id(base.hash().as_bytes())
            ),
            None => format!("On {}, with nothing committed yet", state.branch),
        })
    }));
    let repository = create_memo(clone!(state -> move || {
        state.with(|state| {
            state
                .repository
                .map(|repository| ChildTarget::new(repository, Repository::CONTENT_TYPE))
        })
    }));
    let idle = create_memo(clone!(status editable -> move || {
        editable.get() && !status.with(|status| status.busy)
    }));
    let blocked = create_memo(clone!(idle -> move || !idle.get()));
    let note = create_memo(clone!(status -> move || {
        status.with(|status| match (status.busy, &status.error) {
            (true, _) => "Working…".to_owned(),
            (false, Some(error)) => error.clone(),
            (false, None) => String::new(),
        })
    }));
    let has_note = create_memo(clone!(note -> move || !note.with(String::is_empty)));
    let behind = create_memo(clone!(status -> move || status.with(|status| status.behind)));
    let changes = create_memo(clone!(status -> move || {
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
    }));
    let unchanged = create_memo(clone!(changes -> move || changes.with(Vec::is_empty)));
    let conflicts = create_memo(clone!(state -> move || {
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
    }));
    let conflicted = create_memo(clone!(conflicts -> move || !conflicts.with(Vec::is_empty)));
    let branches = create_memo(clone!(status state -> move || {
        let current = state.with(|state| state.branch.clone());
        status.with(|status| {
            status
                .branches
                .iter()
                .map(|branch| (branch.name.clone(), branch.name == current))
                .collect::<Vec<_>>()
        })
    }));
    let log = create_memo(clone!(status -> move || status.with(|status| status.log.clone())));

    let (message, set_message) = create_signal(String::new());
    let commit_blocked = create_memo(clone!(blocked unchanged message conflicted -> move || {
        blocked.get()
            || unchanged.get()
            || conflicted.get()
            || message.with(|message| message.trim().is_empty())
    }));
    let (branch_name, set_branch_name) = create_signal(String::new());
    let branch_blocked = create_memo(clone!(blocked branch_name -> move || {
        blocked.get() || branch_name.with(|name| name.trim().is_empty())
    }));

    let updating = editor.clone();
    let committing = editor.clone();
    let commit_message = message.clone();
    let clear_message = set_message.clone();
    let branching = editor.clone();
    let new_branch = branch_name.clone();
    let clear_branch = set_branch_name.clone();
    let changing = editor.clone();
    let change_rows = move |row: ChangeRow| {
        let editor = changing.clone();
        view! {
            <ChangeLine editor row />
        }
    };
    let resolving = editor.clone();
    let resolve_blocked = blocked.clone();
    let conflict_rows = move |row: ConflictRow| {
        let editor = resolving.clone();
        let blocked = resolve_blocked.clone();
        view! {
            <ConflictLine editor blocked row />
        }
    };
    let switching = editor.clone();
    let switch_blocked = blocked.clone();
    let branch_rows = move |(name, current): (String, bool)| {
        let editor = switching.clone();
        let blocked = switch_blocked.clone();
        view! {
            <BranchLine editor blocked name current />
        }
    };

    let theme = use_theme();
    view! {
        <Frame color={theme.background.clone()} padding_horizontal=PADDING padding_vertical=PADDING>
            <List spacing=0.0>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <List spacing=SECTION_SPACING>
                        <List spacing=6.0>
                            <Heading content="Checkout" />
                            <Caption content={summary} @test_id={"checkout.summary"} />
                            <BlockLink
                                editor={editor.clone()}
                                block={repository}
                                @test_id={"checkout.repository"}
                            />
                            <Show condition={has_note}>
                                <Caption content={note} wrap=true @test_id={"checkout.note"} />
                            </Show>
                        </List>
                        <Show condition={behind}>
                            <Card>
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=10.0
                                >
                                    <Body
                                        @sizing=ItemSize::Percent(100.0)
                                        content="The branch has moved on since this checkout's base."
                                    />
                                    <Button
                                        label="Bring changes in"
                                        variant=ButtonVariant::Primary
                                        disabled={blocked.clone()}
                                        @test_id={"checkout.update"}
                                        on_click={move || updating.version(VersionCommand::Update)}
                                    />
                                </List>
                            </Card>
                        </Show>
                        <Show condition={conflicted}>
                            <Card>
                                <List spacing=12.0>
                                    <Heading content="Conflicts" />
                                    <ForEach keys={conflicts} view={conflict_rows} />
                                </List>
                            </Card>
                        </Show>
                        <Card>
                            <List spacing=10.0>
                                <Heading content="Changes" />
                                <Show condition={unchanged}>
                                    <Caption content="Nothing has changed since the last commit." />
                                </Show>
                                <ForEach keys={changes} view={change_rows} />
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=10.0
                                >
                                    <TextInput
                                        @sizing=ItemSize::Percent(100.0)
                                        value={message}
                                        placeholder="What changed?"
                                        label="Commit message"
                                        @test_id={"checkout.message"}
                                        on_change={move |value| set_message.set(value)}
                                    />
                                    <Button
                                        label="Commit"
                                        variant=ButtonVariant::Primary
                                        disabled={commit_blocked}
                                        @test_id={"checkout.commit"}
                                        on_click={move || {
                                            let message = commit_message.get().trim().to_owned();
                                            committing.version(VersionCommand::Commit { message });
                                            clear_message.set(String::new());
                                        }}
                                    />
                                </List>
                            </List>
                        </Card>
                        <Card>
                            <List spacing=10.0>
                                <Heading content="Branches" />
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
                                        on_change={move |value| set_branch_name.set(value)}
                                    />
                                    <Button
                                        label="Make branch"
                                        variant=ButtonVariant::Secondary
                                        disabled={branch_blocked}
                                        @test_id={"checkout.make-branch"}
                                        on_click={move || {
                                            let name = new_branch.get().trim().to_owned();
                                            branching.version(VersionCommand::CreateBranch { name });
                                            clear_branch.set(String::new());
                                        }}
                                    />
                                </List>
                            </List>
                        </Card>
                        <Card>
                            <List spacing=10.0>
                                <Heading content="History" />
                                <VersionHistory log={log} />
                            </List>
                        </Card>
                    </List>
                </Scroll>
            </List>
        </Frame>
    }
}

#[component]
fn ChangeLine(editor: Editor, row: ChangeRow) -> NodeId {
    let (block, block_type, name, label) = row;
    let removed = label == change_label(VersionChangeKind::Removed);
    let shown = match removed {
        true => {
            let name = name.unwrap_or_else(|| "Untitled".to_owned());
            view! {
                <Body content={name} />
            }
        }
        false => {
            let target = Some(ChildTarget::new(block, block_type));
            view! {
                <BlockLink editor block={target} />
            }
        }
    };
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=10.0
            @test_id={format!("checkout.change.{block}")}
        >
            <Caption content={label} />
            {shown}
        </List>
    }
}

#[component]
fn ConflictLine(editor: Editor, blocked: Memo<bool>, row: ConflictRow) -> NodeId {
    let (block, block_type, label, base, ours, theirs) = row;
    let side = |side: Option<Uuid>| side.map(|side| ChildTarget::new(side, block_type));
    let choose = |take: ConflictSide| {
        let editor = editor.clone();
        move || {
            editor.version(VersionCommand::Resolve {
                block_id: block.into_bytes(),
                take,
            });
        }
    };
    let sides = match base.is_some() || ours.is_some() || theirs.is_some() {
        true => {
            let (base_editor, ours_editor, theirs_editor) =
                (editor.clone(), editor.clone(), editor.clone());
            let (take_base, take_ours, take_theirs) = (
                choose(ConflictSide::Base),
                choose(ConflictSide::Ours),
                choose(ConflictSide::Theirs),
            );
            let (base_blocked, ours_blocked, theirs_blocked) =
                (blocked.clone(), blocked.clone(), blocked.clone());
            let (base, ours, theirs) = (side(base), side(ours), side(theirs));
            view! {
                <List spacing=8.0>
                    <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                        <Caption content="Base" />
                        <BlockLink editor={base_editor} block={base} fallback="none" />
                        <Caption content="Yours" />
                        <BlockLink editor={ours_editor} block={ours} fallback="none" />
                        <Caption content="Theirs" />
                        <BlockLink editor={theirs_editor} block={theirs} fallback="none" />
                    </List>
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Button
                            label="Take base"
                            variant=ButtonVariant::Secondary
                            disabled={base_blocked}
                            @test_id={format!("checkout.base.{block}")}
                            on_click={take_base}
                        />
                        <Button
                            label="Take yours"
                            variant=ButtonVariant::Secondary
                            disabled={ours_blocked}
                            @test_id={format!("checkout.ours.{block}")}
                            on_click={take_ours}
                        />
                        <Button
                            label="Take theirs"
                            variant=ButtonVariant::Secondary
                            disabled={theirs_blocked}
                            @test_id={format!("checkout.theirs.{block}")}
                            on_click={take_theirs}
                        />
                    </List>
                </List>
            }
        }
        false => view! {
            <List spacing=0.0 />
        },
    };
    let keep = choose(ConflictSide::Merged);
    let working = Some(ChildTarget::new(block, block_type));
    let link_editor = editor.clone();
    view! {
        <List spacing=8.0 @test_id={format!("checkout.conflict.{block}")}>
            <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                <BlockLink editor={link_editor} block={working} />
                <Caption content={label} />
            </List>
            {sides}
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Button
                    label="Keep as merged"
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
    let branch = name.clone();
    let disabled = create_memo(move || blocked.get() || current);
    let label = match current {
        true => "Current",
        false => "Switch",
    };
    view! {
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=10.0
            @test_id={format!("checkout.branch.{name}")}
        >
            <Body @sizing=ItemSize::Percent(100.0) content={name.clone()} />
            <Button
                label={label}
                variant=ButtonVariant::Secondary
                disabled={disabled}
                @test_id={format!("checkout.switch.{name}")}
                on_click={move || editor.version(VersionCommand::Switch { branch: branch.clone() })}
            />
        </List>
    }
}
