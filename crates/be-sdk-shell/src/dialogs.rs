use be_sdk_beui::be_block::OrganizationPolicies;
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::icons::{
    ICON_CAST, ICON_CROWN, ICON_LINK, ICON_LOCK, ICON_TRIANGLE_ALERT, ICON_USER, ICON_USER_PLUS,
    ICON_X,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::{
    AccessLevel, AccessListing, BlockParent, ChildId, ChildMode, ChildState, DialogAnswer,
    LinkAdmits, ShareLink, Subregion, SubregionContent,
};
use uuid::Uuid;

use super::share::{Member, Offered, OwnershipOffer, Person, Share, ShareAction};
use super::workspace::Workspace;
use be_sdk_beui::beui::TextAlign;
use be_sdk_beui::beui::text::Phrase;
use beui::{Align, Direction};

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
            <UnreadableLayoutDialog workspace />
        </List>
    }
}

#[component]
fn UnreadableLayoutDialog(workspace: Workspace) -> NodeId {
    let report = workspace.unreadable_layout();
    view! {
        <Dialog
            open={move || report.with(Option::is_some)}
            kind=DialogKind::Alert
            title="The layout of this session could not be read"
            width=Size::Dialog
            id="workspace.layout"
            actions={vec![
                DialogAction::secondary("Copy report", move || {
                    if let Some(report) = report.get_untracked() {
                        copy(Clip::text(report));
                    }
                })
                .named("copy-report"),
                DialogAction::secondary("Exit", move || workspace.exit()).named("exit"),
                DialogAction::destructive("Reset layout", move || workspace.reset_layout())
                    .named("reset"),
            ]}
            on_dismiss={|| {}}
        >
            <Text style=TextStyle::PARAGRAPH>
                "Its windows, tabs and panels were saved in a form this version cannot read, so they are not shown."
            </Text>
            <Text style=TextStyle::PARAGRAPH>
                "Reset layout starts the session with a fresh layout and saves it in place of the old one. Exit leaves the saved layout as it is."
            </Text>
        </Dialog>
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
    view! {
        <Dialog
            open={move || block.with(Option::is_some)}
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
    let tab = use_frame_anchor();
    let block = opened(&workspace, |dialog| match dialog {
        OpenDialog::ArtifactSettings(block) => Some(block),
        _ => None,
    });
    let placed = move || {
        block
            .get()
            .map(|block| SubregionContent::ArtifactSettings { block })
    };
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
        <DialogFrame
            open={move || block.with(Option::is_some)}
            size=DialogSize::Large
            anchor={move || DialogAnchor::Rect(tab.get())}
            on_dismiss={dismiss}
        >||
            <Frame height={move || Length::Size(Size::Custom(height.get()))}>
                <Subregion
                    editor={editor}
                    placed={placed}
                    mode=ChildMode::Live
                    dialog=true
                    @test_id={"artifact-settings.editor"}
                    on_state={answered}
                />
            </Frame>
        </DialogFrame>
    }
}

#[component]
fn ShareDialog(workspace: Workspace) -> NodeId {
    let tab = use_frame_anchor();
    let state = workspace.share;
    let listing = workspace.share_members;
    let me = workspace.host().account_id();

    let open = create_memo(move || state.with(Option::is_some));
    // The block the dialog opened for, kept while it stays open: its title and every action
    // name it.
    let shown = create_signal(None::<Uuid>);
    create_effect(move || {
        let block = state.with(|share| share.as_ref().map(|share| share.block));
        if block.is_none() || shown.get_untracked().is_none() {
            shown.set(block);
        }
    });
    let title = create_memo(move || {
        shown
            .get()
            .map(|block| {
                let name = workspace
                    .info(block)
                    .map(|info| workspace.label(info.id, info.block_type).name)
                    .unwrap_or_default();
                Phrase::new()
                    .text("Share ")
                    .quoted(be_lib_quoted_name::quotable_name(&name))
            })
            .unwrap_or_default()
    });
    let loading = create_memo(move || listing.with(|listing| *listing == Err(Pending::Loading)));
    let error = create_memo(move || {
        listing.with(|listing| match listing {
            Err(Pending::Failed(error)) => Some(error.clone()),
            _ => state.with(|share| share.as_ref().and_then(|share| share.error.clone())),
        })
    });
    let query = create_memo(move || {
        state.with(|share| {
            share
                .as_ref()
                .map(|share| share.query.clone())
                .unwrap_or_default()
        })
    });
    let suggestions = create_memo(move || {
        listing.with(|listing| {
            let Ok(listing) = listing else {
                return None;
            };
            state.with(|share| share.as_ref()?.suggestions(listing, me))
        })
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
    // The levels the block offers and their names: its type's own on a hosted block.
    let offered = create_memo(move || {
        listing.with(|listing| listing.as_ref().map(Offered::of).unwrap_or_default())
    });
    let grantable = create_memo(move || offered.with(Offered::grantable));
    let pending_access = create_memo(move || {
        listing.with(|listing| {
            state.with(|share| {
                share
                    .as_ref()
                    .map(|share| share.pending_access(listing.as_ref().ok()))
            })
        })
    });
    let pending_line = create_memo(move || {
        pending_access
            .get()
            .map(|access| offered.with(|offered| offered.line(access)))
            .unwrap_or_default()
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
        listing.with(|listing| {
            let Ok(listing) = listing else {
                return Vec::new();
            };
            state.with(|share| {
                share
                    .as_ref()
                    .map(|share| share.members(listing, me, name_of))
                    .unwrap_or_default()
            })
        })
    });
    let company = create_memo(move || {
        listing.with(|listing| matches!(listing, Ok(listing) if listing.company))
    });
    let hosting = create_memo(move || {
        listing.with(|listing| match listing {
            Ok(listing) => Share::hosting_note(listing, me),
            _ => None,
        })
    });
    let inside = create_memo(move || {
        listing.with(|listing| match listing {
            Ok(listing) => Share::inside_note(listing),
            _ => None,
        })
    });
    let member_keys = create_memo(move || {
        members
            .get()
            .into_iter()
            .map(|member| member.id)
            .collect::<Vec<_>>()
    });
    let nobody = create_memo(move || listing.with(Result::is_ok) && members.with(Vec::is_empty));
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
    let act = Callback::new(move |action: ShareAction| {
        if let Some(block) = shown.get_untracked() {
            workspace.share_action(block, action);
        }
    });
    // Share links show to a manager once both the people and the links are listed.
    let links = create_memo(move || {
        let manages =
            listing.with(|listing| matches!(listing, Ok(listing) if Share::manages(listing, me)));
        manages.then(|| workspace.share_links.get()).flatten()
    });
    // Whom a link may admit, as the organization's policy allows: none where it allows no links.
    // It follows the policy as it changes, the dialog open or not.
    let policy = workspace
        .editor()
        .effective_settings::<OrganizationPolicies>();
    let admits_choices = create_memo(move || {
        let policy = policy.with(OrganizationPolicies::link_sharing);
        listing.with(|listing| match listing {
            Ok(listing) => Share::link_choices(policy, listing.company),
            Err(_) => Vec::new(),
        })
    });
    // An invitation is offered only where the server would make its share link.
    let invites = create_memo(move || {
        let outside = policy.with(OrganizationPolicies::shares_outside);
        listing.with(|listing| match listing {
            Ok(listing) => Share::invites(outside, listing.company),
            Err(_) => false,
        })
    });
    // Whom the person asked to offer ownership to, as the question to confirm it.
    let transferring = create_memo(move || {
        let id = state.with(|share| share.as_ref()?.transferring)?;
        members.with(|members| {
            members
                .iter()
                .find(|member| member.id == id)
                .map(|member| format!("Offer {} ownership?", member.name))
        })
    });
    // An offer of ownership standing: one made to the person, to accept, or one they made, to
    // withdraw.
    let ownership_offer = create_memo(move || {
        listing.with(|listing| Share::ownership_offer(listing.as_ref().ok()?, me))
    });
    let offered_to_me = create_memo(move || match ownership_offer.get()? {
        OwnershipOffer::ToMe { from } => Some(format!("{from} offered you ownership")),
        OwnershipOffer::ByMe { .. } => None,
    });
    let offered_by_me = create_memo(move || match ownership_offer.get()? {
        OwnershipOffer::ByMe { to } => Some(format!(
            "Ownership offered to {to}. Nothing changes until they accept."
        )),
        OwnershipOffer::ToMe { .. } => None,
    });
    view! {
        <Dialog
            open={open}
            title={title}
            width={Size::Custom(460.0)}
            size=DialogSize::Large
            anchor={move || DialogAnchor::Rect(tab.get())}
            id="share"
            on_dismiss={move || workspace.close_dialog()}
        >
            <List spacing=Space::Md>
                <Show value={error}>|error: ReadSignal<String>|
                    <Text style=TextStyle::CAPTION>{error}</Text>
                </Show>
                <TextInput
                    value={Binding::new(query, move |value: String| act.call(ShareAction::Query(value)))}
                    placeholder="Add contacts by name or email"
                    label="Add contacts"
                    @test_id={"share.query"}
                    on_submit={move |_value: String| act.call(ShareAction::Submit)}
                />
                <Show value={suggestions}>|suggestions: ReadSignal<Vec<Person>>|
                    <Card>
                        <List spacing=Space::Xs>
                            <ForEach
                                value={move || suggestions.with(|suggestions| suggestions.iter().map(|person| person.id).collect::<Vec<_>>())}
                                fallback={|| view! {
                                    <Text style=TextStyle::CAPTION>"No matching contacts."</Text>
                                }}
                            >|id: Uuid| {
                                let person = create_memo(move || {
                                    suggestions.with(|suggestions| suggestions.iter().find(|person| person.id == id).cloned())
                                });
                                view! {
                                    <SuggestionButton person act={act} />
                                }
                            }</ForEach>
                        </List>
                    </Card>
                </Show>
                <Show value={move || pending_keys.with(|keys| !keys.is_empty())}>
                    <List spacing=Space::Md>
                        <List direction=Direction::Horizontal spacing=Space::Sm wrap=true>
                            <ForEach value={pending_keys}>|id: Uuid| {
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
                                    <ForEach value={grantable}>|level: AccessLevel|
                                        <ChoiceOption
                                            value=level
                                            label={move || offered.with(|offered| offered.name(level))}
                                        />
                                    </ForEach>
                                }}
                            />
                            <Spacer />
                            <Button
                                label="Add"
                                glyph=ICON_USER_PLUS
                                variant=ButtonVariant::Primary
                                @test_id={"share.add"}
                                on_click={move || act.call(ShareAction::AddPending)}
                            />
                        </List>
                        <Show value={move || !pending_line.with(String::is_empty)}>
                            <Text style=TextStyle::CAPTION @test_id={"share.access.line"}>
                                {pending_line}
                            </Text>
                        </Show>
                    </List>
                </Show>
                <Show value={invites}>
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
                </Show>
                <Show value={move || invite_link.with(|link| !link.is_empty())}>
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
                <Show value={inside}>|inside: ReadSignal<String>|
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                        <Text size=TextSize::Sm align=TextAlign::Center>
                            <Icon glyph=ICON_TRIANGLE_ALERT />
                        </Text>
                        <Text style=TextStyle::CAPTION @test_id={"share.inside"}>{inside}</Text>
                    </List>
                </Show>
                <Show value={transferring}>|question: ReadSignal<String>|
                    <Card>
                        <List spacing=Space::Md>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Xs
                            >
                                <Text align=TextAlign::Center>
                                    <Icon glyph=ICON_CROWN />
                                </Text>
                                <Text size=TextSize::Lg>{question}</Text>
                            </List>
                            <Text style=TextStyle::CAPTION @test_id={"share.transfer.help"}>
                                "Once they accept, they will manage this block and decide who has access. You will keep edit access, but you can't give anyone access or take ownership back. Only your contacts can be offered ownership."
                            </Text>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Md
                            >
                                <Spacer />
                                <Button
                                    label="Cancel"
                                    variant=ButtonVariant::Secondary
                                    @test_id={"share.transfer.cancel"}
                                    on_click={move || act.call(ShareAction::CancelOwnership)}
                                />
                                <Button
                                    label="Offer ownership"
                                    glyph=ICON_CROWN
                                    variant=ButtonVariant::Primary
                                    @test_id={"share.transfer"}
                                    on_click={move || act.call(ShareAction::ConfirmOwnership)}
                                />
                            </List>
                        </List>
                    </Card>
                </Show>
                <Show value={offered_to_me}>|offer: ReadSignal<String>|
                    <Card>
                        <List spacing=Space::Md>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Xs
                            >
                                <Text align=TextAlign::Center>
                                    <Icon glyph=ICON_CROWN />
                                </Text>
                                <Text size=TextSize::Lg @test_id={"share.offered"}>{offer}</Text>
                            </List>
                            <Text style=TextStyle::CAPTION>
                                "If you accept, you will manage this block and decide who has access. They will keep edit access."
                            </Text>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Md
                            >
                                <Spacer />
                                <Button
                                    label="Accept ownership"
                                    glyph=ICON_CROWN
                                    variant=ButtonVariant::Primary
                                    @test_id={"share.accept"}
                                    on_click={move || act.call(ShareAction::AcceptOwnership)}
                                />
                            </List>
                        </List>
                    </Card>
                </Show>
                <Show value={offered_by_me}>|offer: ReadSignal<String>|
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                        <Text size=TextSize::Sm align=TextAlign::Center>
                            <Icon glyph=ICON_CROWN />
                        </Text>
                        <ListItem grow=1.0>
                            <Text style=TextStyle::CAPTION @test_id={"share.offered"}>{offer}</Text>
                        </ListItem>
                        <Button
                            label="Withdraw"
                            variant=ButtonVariant::Secondary
                            size=ButtonSize::Small
                            @test_id={"share.withdraw"}
                            on_click={move || act.call(ShareAction::WithdrawOwnership)}
                        />
                    </List>
                </Show>
                <Show value={links}>|links: ReadSignal<Vec<ShareLink>>|
                    <LinksSection
                        links
                        listing
                        offered
                        choices={admits_choices}
                        company
                        state
                        me
                        act={act}
                    />
                </Show>
                <Text style=TextStyle::HEADING>"People with access"</Text>
                <Show value={hosting}>|hosting: ReadSignal<String>|
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                        <Text size=TextSize::Sm align=TextAlign::Center>
                            <Icon glyph=ICON_CAST />
                        </Text>
                        <Text style=TextStyle::CAPTION @test_id={"share.hosting"}>{hosting}</Text>
                    </List>
                </Show>
                <Show value={loading}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                        <Progress width=INLINE_PROGRESS_WIDTH />
                        <Text style=TextStyle::CAPTION>"Loading members…"</Text>
                    </List>
                </Show>
                <Show value={company}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                        <Text size=TextSize::Sm align=TextAlign::Center>
                            <Icon glyph=ICON_LOCK />
                        </Text>
                        <ListItem grow=1.0>
                            <Text style=TextStyle::CAPTION @test_id={"share.administrators"}>
                                "The organization's administrators can manage this block. This can't be changed."
                            </Text>
                        </ListItem>
                    </List>
                </Show>
                <Frame height=MEMBERS_HEIGHT>
                    <Scroll>
                        <Show value={nobody}>
                            <Text style=TextStyle::CAPTION>"Nobody can open this block yet."</Text>
                        </Show>
                        <ForEach value={member_keys}>|id: Uuid| {
                            let member = create_memo(move || {
                                members.get().into_iter().find(|member| member.id == id)
                            });
                            view! {
                                <MemberRow member offered act={act} />
                            }
                        }</ForEach>
                    </Scroll>
                </Frame>
            </List>
        </Dialog>
    }
}

/// "Anyone with the link": the share links open on the block, each with the link to copy when
/// this account keeps it and a way to revoke it, and the choices for making one.
#[component]
fn LinksSection(
    links: ReadSignal<Vec<ShareLink>>,
    listing: Memo<Ready<AccessListing>>,
    offered: Memo<Offered>,
    choices: Memo<Vec<LinkAdmits>>,
    company: Memo<bool>,
    state: Signal<Option<Share>>,
    me: Uuid,
    act: ShareAct,
) -> NodeId {
    let ids = create_memo(move || {
        links.with(|links| links.iter().map(|link| link.id.clone()).collect::<Vec<_>>())
    });
    let allowed = create_memo(move || choices.with(|choices| !choices.is_empty()));
    let several = create_memo(move || choices.with(|choices| choices.len() > 1));
    let levels = create_memo(move || offered.with(Offered::link_levels));
    let access = create_memo(move || {
        listing.with(|listing| {
            state.with(|share| {
                share
                    .as_ref()
                    .map(|share| share.link_access(listing.as_ref().ok()))
            })
        })
    });
    let access_line = create_memo(move || {
        access
            .get()
            .map(|access| offered.with(|offered| offered.line(access)))
            .unwrap_or_default()
    });
    let admits = create_memo(move || {
        Some(state.with(|share| {
            share
                .as_ref()
                .map_or(LinkAdmits::Members, |share| share.link_admits)
        }))
    });
    let making = create_memo(move || {
        state.with(|share| {
            share
                .as_ref()
                .is_some_and(|share| share.making_link.is_some())
        })
    });
    let revoking = create_memo(move || {
        state.with(|share| share.as_ref().is_some_and(|share| share.revoking.is_some()))
    });
    view! {
        <List spacing=Space::Sm>
            <Text style=TextStyle::HEADING>"Anyone with the link"</Text>
            <ForEach value={ids}>|id: String| {
                let link = create_memo({
                    let id = id.clone();
                    move || links.with(|links| links.iter().find(|link| link.id == id).cloned())
                });
                let pending = create_memo({
                    let id = id.clone();
                    move || {
                        state.with(|share| {
                            share
                                .as_ref()
                                .is_some_and(|share| share.revoking_now.contains(&id))
                        })
                    }
                });
                view! {
                    <LinkRow link company offered pending me act={act} />
                }
            }</ForEach>
            <Show value={revoking}>
                <Card>
                    <List spacing=Space::Md>
                        <Text size=TextSize::Lg>"Revoke this link?"</Text>
                        <Text style=TextStyle::CAPTION @test_id={"share.links.revoke.help"}>
                            "Everyone who opened it loses the access it gave them, even a lower access someone gave them here before. The link stops working. This can't be undone."
                        </Text>
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <Spacer />
                            <Button
                                label="Cancel"
                                variant=ButtonVariant::Secondary
                                @test_id={"share.links.revoke.cancel"}
                                on_click={move || act.call(ShareAction::CancelRevoke)}
                            />
                            <Button
                                label="Revoke link"
                                glyph=ICON_X
                                variant=ButtonVariant::Primary
                                @test_id={"share.links.revoke.confirm"}
                                on_click={move || act.call(ShareAction::ConfirmRevoke)}
                            />
                        </List>
                    </List>
                </Card>
            </Show>
            <Show
                value={allowed}
                fallback={|| view! {
                    <Text style=TextStyle::CAPTION @test_id={"share.links.off"}>
                        "This organization's policy allows no share links."
                    </Text>
                }}
            >
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    spacing=Space::Md
                    wrap=true
                >
                    <Select
                        selected={Binding::new(access, move |access: Option<AccessLevel>| {
                            if let Some(access) = access {
                                act.call(ShareAction::LinkAccess(access));
                            }
                        })}
                        label="Access the link grants"
                        options={view! {
                            <ForEach value={levels}>|level: AccessLevel|
                                <ChoiceOption
                                    value=level
                                    label={move || offered.with(|offered| offered.name(level))}
                                />
                            </ForEach>
                        }}
                    />
                    <Show value={several}>
                        <Select
                            selected={Binding::new(admits, move |admits: Option<LinkAdmits>| {
                                if let Some(admits) = admits {
                                    act.call(ShareAction::LinkAdmits(admits));
                                }
                            })}
                            label="Who can open it"
                            options={view! {
                                <ForEach value={choices}>|admits: LinkAdmits|
                                    <ChoiceOption
                                        value=admits
                                        label={Share::admits_label(admits)}
                                    />
                                </ForEach>
                            }}
                        />
                    </Show>
                    <Spacer />
                    <Button
                        label="Make link"
                        glyph=ICON_LINK
                        variant=ButtonVariant::Secondary
                        busy={making}
                        @test_id={"share.links.make"}
                        on_click={move || act.call(ShareAction::MakeLink)}
                    />
                </List>
                <Show value={move || !access_line.with(String::is_empty)}>
                    <Text style=TextStyle::CAPTION @test_id={"share.links.access.line"}>
                        {access_line}
                    </Text>
                </Show>
            </Show>
        </List>
    }
}

#[component]
fn LinkRow(
    link: Memo<Option<ShareLink>>,
    company: Memo<bool>,
    offered: Memo<Offered>,
    pending: Memo<bool>,
    me: Uuid,
    act: ShareAct,
) -> NodeId {
    let described = create_memo(move || {
        link.get()
            .map(|link| {
                offered.with(|offered| Share::describe_link(&link, company.get(), me, offered))
            })
            .unwrap_or_default()
    });
    let url = create_memo(move || link.get().map(|link| link.url).unwrap_or_default());
    let kept = create_memo(move || url.with(|url| !url.is_empty()));
    view! {
        <Card>
            <List spacing=Space::Sm>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Text align=TextAlign::Center>
                        <Icon glyph=ICON_LINK />
                    </Text>
                    <ListItem grow=1.0>
                        <Text style=TextStyle::CAPTION>{described}</Text>
                    </ListItem>
                    <Button
                        label="Revoke"
                        glyph=ICON_X
                        variant=ButtonVariant::Secondary
                        size=ButtonSize::Small
                        busy={pending}
                        @test_id={format!("share.links.revoke.{}", link.with_untracked(|link| link.as_ref().map(|link| link.id.clone()).unwrap_or_default()))}
                        on_click={move || {
                            if let Some(link) = link.get_untracked() {
                                act.call(ShareAction::OfferRevoke(link.id));
                            }
                        }}
                    />
                </List>
                <Show value={kept}>
                    <TextInput
                        value={Binding::new(url, Callback::empty())}
                        label="Share link"
                        copyable=true
                        @test_id={"share.links.url"}
                    />
                </Show>
            </List>
        </Card>
    }
}

#[component]
fn SuggestionButton(person: Memo<Option<Person>>, act: ShareAct) -> NodeId {
    let label = move || {
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
    };
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
fn MemberRow(member: Memo<Option<Member>>, offered: Memo<Offered>, act: ShareAct) -> NodeId {
    let name = create_memo(move || member.get().map(|member| member.name).unwrap_or_default());
    let email = create_memo(move || member.get().map(|member| member.email).unwrap_or_default());
    let fixed = create_memo(move || member.get().and_then(|member| member.fixed));
    let is_fixed = create_memo(move || fixed.with(Option::is_some));
    let note = create_memo(move || member.get().and_then(|member| member.note));
    let access_label = move || {
        member
            .get()
            .map(|member| offered.with(|offered| offered.name(member.access)))
            .unwrap_or_default()
    };
    let choices = create_memo(move || {
        member
            .get()
            .map(|member| offered.with(|offered| member.choices(offered)))
            .unwrap_or_default()
    });
    let selected = create_memo(move || Some(member.get()?.access));
    view! {
        <Frame padding={Padding::vertical(Space::Xs)}>
            <Card>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <ListItem grow=1.0>
                        <List spacing=Space::Xs2>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=Space::Xs
                            >
                                <Text align=TextAlign::Center>
                                    <Icon glyph=ICON_USER />
                                </Text>
                                <Text size=TextSize::Lg>{name}</Text>
                            </List>
                            <Text style=TextStyle::CAPTION>{email}</Text>
                            <Show value={fixed}>|fixed: ReadSignal<String>|
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=Space::Xs
                                >
                                    <Text size=TextSize::Sm align=TextAlign::Center>
                                        <Icon glyph=ICON_LOCK />
                                    </Text>
                                    <Text style=TextStyle::CAPTION>{fixed}</Text>
                                </List>
                            </Show>
                            <Show value={note}>|note: ReadSignal<String>|
                                <Text style=TextStyle::CAPTION>{note}</Text>
                            </Show>
                        </List>
                    </ListItem>
                    <Show
                        value={is_fixed}
                        fallback={move || view! {
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
                                    <ForEach value={choices}>|level: AccessLevel|
                                        <ChoiceOption
                                            value=level
                                            label={move || offered.with(|offered| offered.name(level))}
                                        />
                                    </ForEach>
                                }}
                            />
                        }}
                    >
                        <Button
                            label={access_label}
                            variant=ButtonVariant::Secondary
                            disabled=true
                            on_click={|| {}}
                        />
                    </Show>
                    <Show
                        value={move || member.with(|member| member.as_ref().is_some_and(|member| member.may_own))}
                    >
                        <Button
                            icon_only=true
                            glyph=ICON_CROWN
                            label="Offer ownership"
                            size=ButtonSize::Small
                            @test_id={format!("share.owner.{}", member.with_untracked(|member| member.as_ref().map(|member| member.id).unwrap_or_default()))}
                            on_click={move || {
                                if let Some(member) = member.get_untracked() {
                                    act.call(ShareAction::OfferOwnership(member.id));
                                }
                            }}
                        />
                    </Show>
                    <Show
                        value={move || member.with(|member| member.as_ref().is_some_and(|member| member.removable))}
                    >
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
