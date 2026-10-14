
use block_editor_beui::be_block::metadata::MAX_NAME_BYTES;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_CLOSE, ICON_LOCK, ICON_PERSON, ICON_PERSON_ADD, ICON_REFRESH,
};
use block_editor_beui::beui::reactive::{
    Align, Callback, Direction, ForEach, Frame, ItemSize, Justify, List, Memo, Show, Spacer, component, create_effect, create_memo, create_signal, untrack, view,
};
use block_editor_beui::beui::styled::{
    Button, ButtonVariant, Caption, Card, Chip, Dialog, Heading, Icon, IconButton, IconButtonSize,
    Paragraph, Scroll, Select, Spinner, TextInput,
};
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::{
    AccessLevel, BlockParent, ChildMode, ChildState, Subregion, SubregionContent,
};
use uuid::Uuid;

use super::share::{GRANTABLE, Member, Person, ShareAction};
use super::workspace::Workspace;

const MEMBERS_HEIGHT: f32 = 280.0;
const SETTINGS_HEIGHT: f32 = 96.0;

type ShareAct = Callback<ShareAction>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenDialog {
    Rename(Uuid),
    Share(Uuid),
    ArtifactSettings(Uuid),
    Unlink(Uuid),
}

#[component]
pub fn WorkspaceDialogs(workspace: Workspace) -> NodeId {
    view! {
        <List spacing=0.0>
            <RenameDialog workspace={workspace} />
            <ShareDialog workspace={workspace} />
            <ArtifactSettingsDialog workspace={workspace} />
            <UnlinkDialog workspace />
        </List>
    }
}

fn opened(workspace: &Workspace, of: fn(OpenDialog) -> Option<Uuid>) -> Memo<Option<Uuid>> {
    let dialog = workspace.dialog;
    create_memo(move || dialog.get().and_then(of))
}

#[component]
fn RenameDialog(workspace: Workspace) -> NodeId {
    let block = opened(&workspace, |dialog| match dialog {
        OpenDialog::Rename(block) => Some(block),
        _ => None,
    });
    let open = create_memo(move || block.get().is_some());
    let (name, set_name) = create_signal(String::new());
    let initial = create_memo(move || {
        block
            .get()
            .and_then(|block| workspace.info(block))
            .map(|info| workspace.label(info.id, info.block_type).name)
            .unwrap_or_default()
    });
    create_effect(move || {
        let initial = initial.get();
        untrack(|| set_name.set(initial));
    });
    let invalid = create_memo(move || name.get().len() > MAX_NAME_BYTES);
    let error = create_memo(move || {
        if invalid.get() {
            format!("Name must be at most {MAX_NAME_BYTES} UTF-8 bytes.")
        } else {
            Default::default()
        }
    });
    let submit = move || {
        if let Some(block) = block.get_untracked()
            && !invalid.get_untracked()
        {
            workspace.rename(block, &name.get_untracked());
        }
    };
    view! {
        <Dialog open={open} title="Rename block" on_dismiss={move || workspace.close_dialog()}>
            <List spacing=10.0>
                <TextInput
                    value={name}
                    label="Name"
                    focused=true
                    @test_id={"rename.name"}
                    on_change={move |value: String| set_name.set(value)}
                    on_submit={move |_value: String| submit()}
                />
                <Show condition={invalid}>
                    <Caption content={error} />
                </Show>
                <List direction=Direction::Horizontal spacing=8.0>
                    <Button
                        label="Rename"
                        variant=ButtonVariant::Primary
                        disabled={invalid}
                        @test_id={"rename.submit"}
                        on_click={move || submit()}
                    />
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={move || workspace.close_dialog()}
                    />
                </List>
            </List>
        </Dialog>
    }
}

#[component]
fn UnlinkDialog(workspace: Workspace) -> NodeId {
    let block = opened(&workspace, |dialog| match dialog {
        OpenDialog::Unlink(block) => Some(block),
        _ => None,
    });
    let open = create_memo(move || block.get().is_some());
    view! {
        <Dialog
            open={open}
            title="Unlink from the source block?"
            width=360.0
            on_dismiss={move || workspace.close_dialog()}
        >
            <List spacing=12.0>
                <Paragraph
                    content="This block keeps what was generated for it, but stops being rebuilt from its source and becomes editable."
                />
                <Paragraph content="The link and its settings cannot be restored." />
                <List direction=Direction::Horizontal spacing=8.0>
                    <Button
                        label="Unlink"
                        variant=ButtonVariant::Primary
                        @test_id={"workspace.confirm"}
                        on_click={move || {
                            if let Some(block) = block.get_untracked() {
                                workspace.host().unlink_artifact(block);
                            }
                            workspace.close_dialog();
                        }}
                    />
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={move || workspace.close_dialog()}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                </List>
            </List>
        </Dialog>
    }
}

#[component]
fn ArtifactSettingsDialog(workspace: Workspace) -> NodeId {
    let block = opened(&workspace, |dialog| match dialog {
        OpenDialog::ArtifactSettings(block) => Some(block),
        _ => None,
    });
    let open = create_memo(move || block.get().is_some());
    let placed = create_memo(move || {
        block
            .get()
            .map(|block| SubregionContent::ArtifactSettings { block })
    });
    let (state, set_state) = create_signal(ChildState::default());
    let unchanged = create_memo(move || {
        !state.with(|state| {
            state
                .settings
                .as_ref()
                .is_some_and(|settings| settings.changed)
        })
    });
    let summary = create_memo(move || {
        state.with(|state| {
            state
                .settings
                .as_ref()
                .and_then(|settings| settings.summary.clone())
        })
    });
    let has_summary = create_memo(move || summary.get().is_some());
    let summary_text = create_memo(move || summary.get().unwrap_or_default());
    let height = create_memo(move || {
        state.with(|state| {
            state
                .intrinsic_size
                .map_or(SETTINGS_HEIGHT, |size| size.y.max(1.0))
        })
    });
    let editor = workspace.editor();
    view! {
        <Dialog
            open={open}
            title="Dynamic artifact settings"
            width=360.0
            on_dismiss={move || workspace.close_dialog()}
        >
            <List spacing=12.0>
                <Frame height={height}>
                    <Subregion
                        editor={editor}
                        placed={placed}
                        mode=ChildMode::Live
                        @test_id={"artifact-settings.editor"}
                        on_state={move |next: ChildState| set_state.set(next)}
                    />
                </Frame>
                <Show condition={has_summary}>
                    <Caption content={summary_text} />
                </Show>
                <List direction=Direction::Horizontal spacing=8.0>
                    <Button
                        label="Apply"
                        variant=ButtonVariant::Primary
                        disabled={unchanged}
                        @test_id={"artifact-settings.apply"}
                        on_click={move || {
                            if let Some(child) = state.with_untracked(|state| state.child) {
                                workspace.host().commit_child(child, BlockParent::Root, None);
                            }
                            workspace.close_dialog();
                        }}
                    />
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={move || workspace.close_dialog()}
                    />
                </List>
            </List>
        </Dialog>
    }
}

#[component]
fn ShareDialog(workspace: Workspace) -> NodeId {
    let state = workspace.share;
    let me = workspace.host().account_id();
    let open = create_memo(move || state.with(Option::is_some));
    let title = create_memo(move || {
        state
            .get()
            .map(|share| {
                let name = workspace
                    .info(share.block)
                    .map(|info| workspace.label(info.id, info.block_type).name)
                    .unwrap_or_default();
                format!("Share \u{201c}{name}\u{201d}")
            })
            .unwrap_or_default()
    });
    let loading = create_memo(move || {
        state.with(|share| share.as_ref().is_some_and(|share| share.loading()))
    });
    let refreshing = create_memo(move || {
        state.with(|share| share.as_ref().is_some_and(|share| share.request.is_some()))
    });
    let error = create_memo(move || {
        state.with(|share| share.as_ref().and_then(|share| share.error.clone()))
    });
    let has_error = create_memo(move || error.get().is_some());
    let error_text = create_memo(move || error.get().unwrap_or_default());
    let query = create_memo(move || {
        state.with(|share| {
            share
                .as_ref()
                .map(|share| share.query.clone())
                .unwrap_or_default()
        })
    });
    let suggestions = create_memo(move || {
        state.with(|share| share.as_ref().and_then(|share| share.suggestions(me)))
    });
    let searching = create_memo(move || suggestions.get().is_some());
    let unmatched = create_memo(move || {
        suggestions
            .get()
            .is_some_and(|suggestions| suggestions.is_empty())
    });
    let suggestion_keys = create_memo(move || {
        suggestions
            .get()
            .unwrap_or_default()
            .into_iter()
            .map(|person| person.id)
            .collect::<Vec<_>>()
    });
    let pending = create_memo(move || {
        state.with(|share| {
            share
                .as_ref()
                .map(|share| share.pending.clone())
                .unwrap_or_default()
        })
    });
    let pending_keys = create_memo(move || {
        pending
            .get()
            .into_iter()
            .map(|person| person.id)
            .collect::<Vec<_>>()
    });
    let has_pending = create_memo(move || !pending_keys.get().is_empty());
    let pending_access = create_memo(move || {
        state.with(|share| {
            share.as_ref().map_or(Some(0), |share| {
                GRANTABLE
                    .iter()
                    .position(|access| *access == share.pending_access)
            })
        })
    });
    let members = create_memo(move || {
        state.with(|share| {
            share
                .as_ref()
                .map(|share| share.members(me))
                .unwrap_or_default()
        })
    });
    let member_keys = create_memo(move || {
        members
            .get()
            .into_iter()
            .map(|member| member.id)
            .collect::<Vec<_>>()
    });
    let nobody = create_memo(move || {
        state.with(|share| share.as_ref().is_some_and(|share| share.loaded))
            && members.get().is_empty()
    });
    let act = Callback::new(move |action: ShareAction| workspace.share_action(action));
    view! {
        <Dialog open={open} title={title} width=460.0 on_dismiss={move || workspace.close_dialog()}>
            <List spacing=10.0>
                <Show condition={has_error}>
                    <Caption content={error_text} />
                </Show>
                <Show condition={loading}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Spinner />
                        <Caption content="Loading members…" />
                    </List>
                </Show>
                <TextInput
                    value={query}
                    placeholder="Add people by name or email"
                    label="Add people"
                    @test_id={"share.query"}
                    on_change={move |value: String| act.call(ShareAction::Query(value))}
                    on_submit={move |_value: String| act.call(ShareAction::Submit)}
                />
                <Show condition={searching}>
                    {move || view! {
                        <Card>
                            <List spacing=4.0>
                                <Show condition={unmatched}>
                                    <Caption content="No matching workspace members." />
                                </Show>
                                <ForEach keys={suggestion_keys}>
                                    {move |id: Uuid| {
                                        let person = create_memo(move || {
                                            suggestions
                                                .get()
                                                .unwrap_or_default()
                                                .into_iter()
                                                .find(|person| person.id == id)
                                        });
                                        view! {
                                            <SuggestionButton person act={act} />
                                        }
                                    }}
                                </ForEach>
                            </List>
                        </Card>
                    }}
                </Show>
                <Show condition={has_pending}>
                    {move || view! {
                        <List spacing=8.0>
                            <List direction=Direction::Horizontal spacing=6.0 wrap=true>
                                <ForEach keys={pending_keys}>
                                    {move |id: Uuid| {
                                        let name = create_memo(move || {
                                            pending
                                                .get()
                                                .into_iter()
                                                .find(|person| person.id == id)
                                                .map(|person| person.name)
                                                .unwrap_or_default()
                                        });
                                        view! {
                                            <List
                                                direction=Direction::Horizontal
                                                align=Align::Center
                                                spacing=2.0
                                            >
                                                <Chip label={name} />
                                                <IconButton
                                                    glyph={ICON_CLOSE.to_owned()}
                                                    label="Do not add"
                                                    size=IconButtonSize::Compact
                                                    on_click={move || act.call(ShareAction::Unpick(id))}
                                                />
                                            </List>
                                        }
                                    }}
                                </ForEach>
                            </List>
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <Select
                                    selected={pending_access}
                                    label="Access to grant"
                                    on_change={move |index: Option<usize>| {
                                        if let Some(access) = index.and_then(|index| GRANTABLE.get(index)) {
                                            act.call(ShareAction::PendingAccess(*access));
                                        }
                                    }}
                                    options={view! {
                                        <ChoiceOption label={AccessLevel::Edit.label()} />
                                        <ChoiceOption label={AccessLevel::View.label()} />
                                        <ChoiceOption label={AccessLevel::KnowExists.label()} />
                                    }}
                                />
                                <Spacer @sizing=ItemSize::Percent(100.0) />
                                <Button
                                    label="Add"
                                    glyph={ICON_PERSON_ADD.to_owned()}
                                    variant=ButtonVariant::Primary
                                    @test_id={"share.add"}
                                    on_click={move || act.call(ShareAction::AddPending)}
                                />
                            </List>
                        </List>
                    }}
                </Show>
                <Heading content="People with access" />
                <Frame height=MEMBERS_HEIGHT>
                    <Scroll>
                        <Show condition={nobody}>
                            <Caption content="Nobody can open this block yet." />
                        </Show>
                        <ForEach keys={member_keys}>
                            {move |id: Uuid| {
                                let member = create_memo(move || {
                                    members.get().into_iter().find(|member| member.id == id)
                                });
                                view! {
                                    <MemberRow member act={act} />
                                }
                            }}
                        </ForEach>
                    </Scroll>
                </Frame>
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    justify=Justify::SpaceBetween
                    spacing=8.0
                >
                    <Button
                        label="Refresh"
                        glyph={ICON_REFRESH.to_owned()}
                        variant=ButtonVariant::Secondary
                        disabled={refreshing}
                        on_click={move || act.call(ShareAction::Refresh)}
                    />
                    <Button
                        label="Done"
                        variant=ButtonVariant::Primary
                        on_click={move || workspace.close_dialog()}
                    />
                </List>
            </List>
        </Dialog>
    }
}

#[component]
fn SuggestionButton(person: Memo<Option<Person>>, act: ShareAct) -> NodeId {
    let label = create_memo(move || {
        person
            .get()
            .map(|person| format!("{} ({})", person.name, person.email))
            .unwrap_or_default()
    });
    view! {
        <Button
            label={label}
            glyph={ICON_PERSON.to_owned()}
            variant=ButtonVariant::Ghost
            on_click={move || {
                if let Some(person) = person.get_untracked() {
                    act.call(ShareAction::Pick(person.id));
                }
            }}
        />
    }
}

#[component]
fn MemberRow(member: Memo<Option<Member>>, act: ShareAct) -> NodeId {
    let name = create_memo(move || member.get().map(|member| member.name).unwrap_or_default());
    let email = create_memo(move || member.get().map(|member| member.email).unwrap_or_default());
    let fixed = create_memo(move || member.get().and_then(|member| member.fixed));
    let is_fixed = create_memo(move || fixed.get().is_some());
    let editable = create_memo(move || !is_fixed.get());
    let fixed_text = create_memo(move || fixed.get().unwrap_or_default());
    let note = create_memo(move || member.get().and_then(|member| member.note));
    let has_note = create_memo(move || note.get().is_some());
    let note_text = create_memo(move || note.get().unwrap_or_default());
    let access_label = create_memo(move || {
        member
            .get()
            .map(|member| member.access.label().to_owned())
            .unwrap_or_default()
    });
    let selected = create_memo(move || {
        member
            .get()
            .and_then(|member| GRANTABLE.iter().position(|access| *access == member.access))
    });
    let removable = create_memo(move || member.get().is_some_and(|member| member.removable));
    view! {
        <Frame padding_vertical=4.0>
            <Card>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <List @sizing=ItemSize::Percent(100.0) spacing=2.0>
                        <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                            <Icon glyph={ICON_PERSON.to_owned()} />
                            <Heading content={name} />
                        </List>
                        <Caption content={email} />
                        <Show condition={is_fixed}>
                            {move || view! {
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=4.0
                                >
                                    <Icon glyph={ICON_LOCK.to_owned()} text_size=12.0 />
                                    <Caption content={fixed_text} />
                                </List>
                            }}
                        </Show>
                        <Show condition={has_note}>
                            <Caption content={note_text} />
                        </Show>
                    </List>
                    <Show condition={is_fixed}>
                        <Button
                            label={access_label}
                            variant=ButtonVariant::Secondary
                            disabled=true
                            on_click={|| {}}
                        />
                    </Show>
                    <Show condition={editable}>
                        {move || view! {
                            <Select
                                selected={selected}
                                label="Access"
                                on_change={move |index: Option<usize>| {
                                    let Some(member) = member.get_untracked() else {
                                        return;
                                    };
                                    let Some(access) = index.and_then(|index| GRANTABLE.get(index).copied()) else {
                                        return;
                                    };
                                    if access != member.access {
                                        act.call(ShareAction::SetAccess(member.id, access));
                                    }
                                }}
                                options={view! {
                                    <ChoiceOption label={AccessLevel::Edit.label()} />
                                    <ChoiceOption label={AccessLevel::View.label()} />
                                    <ChoiceOption label={AccessLevel::KnowExists.label()} />
                                }}
                            />
                        }}
                    </Show>
                    <Show condition={removable}>
                        {move || view! {
                            <IconButton
                                glyph={ICON_CLOSE.to_owned()}
                                label="Remove access"
                                size=IconButtonSize::Compact
                                on_click={move || {
                                    if let Some(member) = member.get_untracked() {
                                        act.call(ShareAction::SetAccess(member.id, AccessLevel::None));
                                    }
                                }}
                            />
                        }}
                    </Show>
                </List>
            </Card>
        </Frame>
    }
}
