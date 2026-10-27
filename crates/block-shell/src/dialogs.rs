use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{
    ICON_LINK, ICON_LOCK, ICON_TRIANGLE_ALERT, ICON_USER, ICON_USER_PLUS, ICON_X,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{
    AccessLevel, BlockParent, ChildId, ChildMode, ChildState, DialogAnswer, Subregion,
    SubregionContent,
};
use uuid::Uuid;

use super::share::{GRANTABLE, Member, Person, Share, ShareAction};
use super::workspace::Workspace;
use beui::{Align, Direction, ItemSize};
use block_editor_beui::beui::TextAlign;

const MEMBERS_HEIGHT: Size = Size::Custom(280.0);
const SETTINGS_HEIGHT: f32 = 96.0;

type ShareAct = Callback<ShareAction>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenDialog {
    Share(Uuid),
    ArtifactSettings(Uuid),
    Unlink(Uuid),
}

#[component]
pub fn WorkspaceDialogs(workspace: Workspace) -> NodeId {
    view! {
        <List spacing=Space::None>
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
fn UnlinkDialog(workspace: Workspace) -> NodeId {
    let block = opened(&workspace, |dialog| match dialog {
        OpenDialog::Unlink(block) => Some(block),
        _ => None,
    });
    let open = create_memo(move || block.get().is_some());
    view! {
        <Dialog
            open={open}
            kind=DialogKind::Alert
            title="Unlink from the source block?"
            width=Size::Dialog
            id="workspace"
            actions={vec![
                DialogAction::destructive("Unlink", move || {
                    if let Some(block) = block.get_untracked() {
                        workspace.host().unlink_artifact(block);
                    }
                    workspace.close_dialog();
                }),
                DialogAction::cancel("Cancel"),
            ]}
            on_dismiss={move || workspace.close_dialog()}
        >
            <Text style=TextStyle::PARAGRAPH>
                "This block keeps what was generated for it, but stops being rebuilt from its source and becomes editable."
            </Text>
            <Text style=TextStyle::PARAGRAPH>"The link and its settings cannot be restored."</Text>
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
    let child = create_signal(None::<ChildId>);
    let height = create_signal(SETTINGS_HEIGHT);
    let answered = Callback::new(move |state: ChildState| {
        child.set(state.child);
        if let Some(size) = state.intrinsic_size {
            height.set(size.y.max(1.0));
        }
        match state.dialog.answer {
            Some(DialogAnswer::Accepted) => {
                if let Some(child) = state.child {
                    workspace
                        .host()
                        .commit_child(child, BlockParent::Root, None);
                }
                workspace.close_dialog();
            }
            Some(DialogAnswer::Dismissed) => workspace.close_dialog(),
            None => {}
        }
    });
    let dismiss = move || match child.get_untracked() {
        Some(child) => workspace.host().dismiss_child(child),
        None => workspace.close_dialog(),
    };
    let editor = workspace.editor();
    view! {
        <DialogFrame open={open} on_dismiss={dismiss}>|| {
            let height = create_memo(move || Length::Size(Size::Custom(height.get())));
            view! {
                <Frame height>
                    <Subregion
                        editor={editor}
                        placed={placed}
                        mode=ChildMode::Live
                        dialog=true
                        @test_id={"artifact-settings.editor"}
                        on_state={answered}
                    />
                </Frame>
            }
        }</DialogFrame>
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
        state.with(|share| {
            share
                .as_ref()
                .is_some_and(|share| share.members == Err(Pending::Loading))
        })
    });
    let error = create_memo(move || {
        state.with(|share| {
            share.as_ref().and_then(|share| match &share.members {
                Err(Pending::Failed(error)) => Some(error.clone()),
                _ => share.error.clone(),
            })
        })
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
            Some(
                share
                    .as_ref()
                    .map_or(AccessLevel::Edit, |share| share.pending_access),
            )
        })
    });
    let name_of = move |id: Uuid| {
        workspace
            .info(id)
            .filter(|info| info.access.can_view())
            .map_or_else(
                || "a folder above".to_owned(),
                |info| workspace.label(info.id, info.block_type).name,
            )
    };
    let members = create_memo(move || {
        state.with(|share| {
            share
                .as_ref()
                .map(|share| share.members(me, name_of))
                .unwrap_or_default()
        })
    });
    let company = create_memo(move || {
        state.with(|share| {
            share
                .as_ref()
                .is_some_and(|share| share.company && share.members.is_ok())
        })
    });
    let inside =
        create_memo(move || state.with(|share| share.as_ref().and_then(Share::inside_note)));
    let has_inside = create_memo(move || inside.get().is_some());
    let inside_text = create_memo(move || inside.get().unwrap_or_default());
    let member_keys = create_memo(move || {
        members
            .get()
            .into_iter()
            .map(|member| member.id)
            .collect::<Vec<_>>()
    });
    let nobody = create_memo(move || {
        state.with(|share| share.as_ref().is_some_and(|share| share.members.is_ok()))
            && members.get().is_empty()
    });
    let inviting = create_memo(move || {
        state.with(|share| share.as_ref().is_some_and(|share| share.inviting.is_some()))
    });
    let invite_link = create_memo(move || {
        state.with(|share| {
            share
                .as_ref()
                .and_then(|share| share.invite_link.clone())
                .unwrap_or_default()
        })
    });
    let invited = create_memo(move || !invite_link.get().is_empty());
    let act = Callback::new(move |action: ShareAction| workspace.share_action(action));
    view! {
        <Dialog
            open={open}
            title={title}
            width={Size::Custom(460.0)}
            id="share"
            on_dismiss={move || workspace.close_dialog()}
        >
            <List spacing=Space::Md>
                <Show condition={has_error}>
                    <Text style=TextStyle::CAPTION>{error_text}</Text>
                </Show>
                <Show condition={loading}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <Progress width=INLINE_PROGRESS_WIDTH />
                        <Text style=TextStyle::CAPTION>"Loading members…"</Text>
                    </List>
                </Show>
                <TextInput
                    value={Binding::new(query, move |value: String| act.call(ShareAction::Query(value)))}
                    placeholder="Add contacts by name or email"
                    label="Add contacts"
                    @test_id={"share.query"}
                    on_submit={move |_value: String| act.call(ShareAction::Submit)}
                />
                <Show condition={searching}>
                    <Card>
                        <List spacing=Space::Xs>
                            <Show condition={unmatched}>
                                <Text style=TextStyle::CAPTION>"No matching contacts."</Text>
                            </Show>
                            <ForEach keys={suggestion_keys}>|id: Uuid| {
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
                            }</ForEach>
                        </List>
                    </Card>
                </Show>
                <Show condition={has_pending}>
                    <List spacing=Space::Md>
                        <List direction=Direction::Horizontal spacing=Space::Sm wrap=true>
                            <ForEach keys={pending_keys}>|id: Uuid| {
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
                                        spacing=Space::Xs2
                                    >
                                        <Chip label={name} />
                                        <Button
                                            icon_only=true
                                            glyph=ICON_X
                                            label="Do not add"
                                            size=ButtonSize::Small
                                            on_click={move || act.call(ShareAction::Unpick(id))}
                                        />
                                    </List>
                                }
                            }</ForEach>
                        </List>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <Select
                                selected={Binding::new(pending_access, move |access: Option<AccessLevel>| {
                                    if let Some(access) = access {
                                        act.call(ShareAction::PendingAccess(access));
                                    }
                                })}
                                label="Access to grant"
                                options={view! {
                                    <ForEach keys={GRANTABLE.to_vec()}>|level: AccessLevel|
                                        <ChoiceOption value=level label={level.label()} />
                                    </ForEach>
                                }}
                            />
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                            <Button
                                label="Add"
                                glyph=ICON_USER_PLUS
                                variant=ButtonVariant::Primary
                                @test_id={"share.add"}
                                on_click={move || act.call(ShareAction::AddPending)}
                            />
                        </List>
                    </List>
                </Show>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Button
                        label="Invite someone new"
                        glyph=ICON_LINK
                        variant=ButtonVariant::Secondary
                        busy={inviting}
                        @test_id={"share.invite"}
                        on_click={move || act.call(ShareAction::InviteNew)}
                    />
                </List>
                <Show condition={invited}>
                    <List spacing=Space::Sm>
                        <Text style=TextStyle::CAPTION @test_id={"share.invite.help"}>
                            "Send this link yourself, through an app you trust. Whoever opens it first becomes your contact and can open this block. It works once, and it is shown only once."
                        </Text>
                        <TextInput
                            value={Binding::new(invite_link, Callback::empty())}
                            label="Contact link"
                            copyable=true
                            @test_id={"share.invite.link"}
                        />
                    </List>
                </Show>
                <Show condition={has_inside}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                        <Text size=TextSize::Sm align=TextAlign::Center>
                            <Icon glyph=ICON_TRIANGLE_ALERT />
                        </Text>
                        <Text style=TextStyle::CAPTION @test_id={"share.inside"}>
                            {inside_text}
                        </Text>
                    </List>
                </Show>
                <Text style=TextStyle::HEADING>"People with access"</Text>
                <Show condition={company}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                        <Text size=TextSize::Sm align=TextAlign::Center>
                            <Icon glyph=ICON_LOCK />
                        </Text>
                        <Text
                            style=TextStyle::CAPTION
                            @test_id={"share.administrators"}
                            @sizing=ItemSize::Percent(100.0)
                        >
                            "The organization's administrators can manage this block. This can't be changed."
                        </Text>
                    </List>
                </Show>
                <Frame height=MEMBERS_HEIGHT>
                    <Scroll>
                        <Show condition={nobody}>
                            <Text style=TextStyle::CAPTION>"Nobody can open this block yet."</Text>
                        </Show>
                        <ForEach keys={member_keys}>|id: Uuid| {
                            let member = create_memo(move || {
                                members.get().into_iter().find(|member| member.id == id)
                            });
                            view! {
                                <MemberRow member act={act} />
                            }
                        }</ForEach>
                    </Scroll>
                </Frame>
            </List>
        </Dialog>
    }
}

#[component]
fn SuggestionButton(person: Memo<Option<Person>>, act: ShareAct) -> NodeId {
    let label = create_memo(move || {
        person
            .get()
            .map(|person| {
                if person.email.is_empty() {
                    person.name
                } else {
                    format!("{} ({})", person.name, person.email)
                }
            })
            .unwrap_or_default()
    });
    view! {
        <Button
            label={label}
            glyph=ICON_USER
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
    let choices = create_memo(move || {
        member
            .get()
            .map(|member| member.choices())
            .unwrap_or_default()
    });
    let selected = create_memo(move || Some(member.get()?.access));
    let removable = create_memo(move || member.get().is_some_and(|member| member.removable));
    view! {
        <Frame padding={Padding::vertical(Space::Xs)}>
            <Card>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <List @sizing=ItemSize::Percent(100.0) spacing=Space::Xs2>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                            <Text align=TextAlign::Center>
                                <Icon glyph=ICON_USER />
                            </Text>
                            <Text style=TextStyle::HEADING>{name}</Text>
                        </List>
                        <Text style=TextStyle::CAPTION>{email}</Text>
                        <Show condition={is_fixed}>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Xs
                            >
                                <Text size=TextSize::Sm align=TextAlign::Center>
                                    <Icon glyph=ICON_LOCK />
                                </Text>
                                <Text style=TextStyle::CAPTION>{fixed_text}</Text>
                            </List>
                        </Show>
                        <Show condition={has_note}>
                            <Text style=TextStyle::CAPTION>{note_text}</Text>
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
                        <Select
                            selected={Binding::new(selected, move |access: Option<AccessLevel>| {
                                let Some(member) = member.get_untracked() else {
                                    return;
                                };
                                let Some(access) = access else {
                                    return;
                                };
                                if let Some(access) = member.choose(access) {
                                    act.call(ShareAction::SetAccess(member.id, access));
                                }
                            })}
                            label="Access"
                            options={view! {
                                <ForEach keys={choices}>|level: AccessLevel|
                                    <ChoiceOption value=level label={level.label()} />
                                </ForEach>
                            }}
                        />
                    </Show>
                    <Show condition={removable}>
                        <Button
                            icon_only=true
                            glyph=ICON_X
                            label="Remove access"
                            size=ButtonSize::Small
                            on_click={move || {
                                if let Some(member) = member.get_untracked() {
                                    act.call(ShareAction::SetAccess(member.id, AccessLevel::None));
                                }
                            }}
                        />
                    </Show>
                </List>
            </Card>
        </Frame>
    }
}
