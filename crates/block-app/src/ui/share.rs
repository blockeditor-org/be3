use beui::NodeId;
use beui::icons::{ICON_CLOSE, ICON_LOCK, ICON_PERSON, ICON_PERSON_ADD, ICON_REFRESH};
use beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, Spacer, clone, component,
    create_memo, view,
};
use beui::styled::{
    Button, ButtonVariant, Caption, Card, Chip, Dialog, Heading, Icon, IconButton, IconButtonSize,
    Scroll, Select, Spinner, TextInput,
};
use beui::unstyled::ChoiceOption;
use block::BlockAccess;
use uuid::Uuid;

use super::onboarding::ErrorText;
use super::{AppViewStore, UiCommand, send};
use crate::share::{GRANTABLE, Member, ShareCommand, ShareView, Suggestion};

const MEMBERS_HEIGHT: f32 = 280.0;

fn share(command: ShareCommand) {
    send(UiCommand::Share(command));
}

#[component]
pub(super) fn ShareWindow(view: AppViewStore) -> NodeId {
    let state = view.share.clone();
    let open = create_memo(clone!(state -> move || state.get().is_some()));
    let title = create_memo(clone!(state -> move || {
        state
            .get()
            .map(|share| format!("Share \u{201c}{}\u{201d}", share.name))
            .unwrap_or_default()
    }));
    let field = |read: fn(&ShareView) -> bool| {
        let state = state.clone();
        create_memo(move || state.get().is_some_and(|share| read(&share)))
    };
    let loading = field(|share| share.loading);
    let refreshing = field(|share| share.refreshing);
    let nobody = field(|share| share.nobody);
    let error = create_memo(clone!(state -> move || state.get().and_then(|share| share.error)));
    let query = create_memo(
        clone!(state -> move || state.get().map(|share| share.query).unwrap_or_default()),
    );
    let suggestions = create_memo(clone!(state -> move || {
        state.get().and_then(|share| share.suggestions)
    }));
    let searching = create_memo(clone!(suggestions -> move || suggestions.get().is_some()));
    let unmatched = create_memo(clone!(suggestions -> move || {
        suggestions.get().is_some_and(|suggestions| suggestions.is_empty())
    }));
    let suggestion_keys = create_memo(clone!(suggestions -> move || {
        suggestions
            .get()
            .unwrap_or_default()
            .into_iter()
            .map(|suggestion| suggestion.id)
            .collect::<Vec<_>>()
    }));
    let pending = create_memo(
        clone!(state -> move || state.get().map(|share| share.pending).unwrap_or_default()),
    );
    let pending_keys = create_memo(clone!(pending -> move || {
        pending.get().into_iter().map(|account| account.id).collect::<Vec<_>>()
    }));
    let has_pending = create_memo(clone!(pending_keys -> move || !pending_keys.get().is_empty()));
    let pending_access = create_memo(clone!(state -> move || {
        state.get().map_or(Some(0), |share| {
            GRANTABLE
                .iter()
                .position(|access| *access == share.pending_access)
        })
    }));
    let members = create_memo(
        clone!(state -> move || state.get().map(|share| share.members).unwrap_or_default()),
    );
    let member_keys = create_memo(clone!(members -> move || {
        members.get().into_iter().map(|member| member.id).collect::<Vec<_>>()
    }));
    view! {
        <Dialog open={open} title={title} width=460.0 on_dismiss={|| share(ShareCommand::Close)}>
            <List spacing=10.0>
                <ErrorText text={error} />
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
                    on_change={|value: String| share(ShareCommand::SetQuery(value))}
                    on_submit={|_value: String| share(ShareCommand::Submit)}
                />
                <Show condition={searching}>
                    <Card>
                        <List spacing=4.0>
                            <Show condition={unmatched}>
                                <Caption content="No matching workspace members." />
                            </Show>
                            <ForEach keys={suggestion_keys}>
                                {move |id: Uuid| {
                                    let suggestions = suggestions.clone();
                                    let suggestion = create_memo(move || {
                                        suggestions
                                            .get()
                                            .unwrap_or_default()
                                            .into_iter()
                                            .find(|suggestion| suggestion.id == id)
                                    });
                                    view! {
                                        <SuggestionButton suggestion />
                                    }
                                }}
                            </ForEach>
                        </List>
                    </Card>
                </Show>
                <Show condition={has_pending}>
                    <List spacing=8.0>
                        <List direction=Direction::Horizontal spacing=6.0 wrap=true>
                            <ForEach keys={pending_keys}>
                                {move |id: Uuid| {
                                    let pending = pending.clone();
                                    let name = create_memo(move || {
                                        pending
                                            .get()
                                            .into_iter()
                                            .find(|account| account.id == id)
                                            .map(|account| account.name)
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
                                                on_click={move || share(ShareCommand::Unpick(id))}
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
                                on_change={|index: Option<usize>| {
                                    if let Some(access) = index.and_then(|index| GRANTABLE.get(index)) {
                                        share(ShareCommand::SetPendingAccess(*access));
                                    }
                                }}
                                options={view! {
                                    <ChoiceOption label={BlockAccess::Edit.label()} />
                                    <ChoiceOption label={BlockAccess::View.label()} />
                                    <ChoiceOption label={BlockAccess::KnowExists.label()} />
                                }}
                            />
                            <Spacer @sizing=ItemSize::Percent(100.0) />
                            <Button
                                label="Add"
                                glyph={ICON_PERSON_ADD.to_owned()}
                                variant=ButtonVariant::Primary
                                on_click={|| share(ShareCommand::AddPending)}
                            />
                        </List>
                    </List>
                </Show>
                <Heading content="People with access" />
                <Frame height=MEMBERS_HEIGHT>
                    <Scroll>
                        <Show condition={nobody}>
                            <Caption content="Nobody can open this block yet." />
                        </Show>
                        <ForEach keys={member_keys}>
                            {move |id: Uuid| {
                                let members = members.clone();
                                let member = create_memo(move || {
                                    members.get().into_iter().find(|member| member.id == id)
                                });
                                view! {
                                    <MemberRow member />
                                }
                            }}
                        </ForEach>
                    </Scroll>
                </Frame>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Button
                        label="Refresh"
                        glyph={ICON_REFRESH.to_owned()}
                        variant=ButtonVariant::Secondary
                        disabled={refreshing}
                        on_click={|| share(ShareCommand::Refresh)}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Button
                        label="Done"
                        variant=ButtonVariant::Primary
                        on_click={|| share(ShareCommand::Close)}
                    />
                </List>
            </List>
        </Dialog>
    }
}

#[component]
fn SuggestionButton(suggestion: Memo<Option<Suggestion>>) -> NodeId {
    let label = create_memo(clone!(suggestion -> move || {
        suggestion
            .get()
            .map(|suggestion| format!("{} ({})", suggestion.name, suggestion.email))
            .unwrap_or_default()
    }));
    view! {
        <Button
            label={label}
            glyph={ICON_PERSON.to_owned()}
            variant=ButtonVariant::Ghost
            on_click={move || {
                if let Some(suggestion) = suggestion.get_untracked() {
                    share(ShareCommand::Pick(suggestion.id));
                }
            }}
        />
    }
}

#[component]
fn MemberRow(member: Memo<Option<Member>>) -> NodeId {
    let name = create_memo(
        clone!(member -> move || member.get().map(|member| member.name).unwrap_or_default()),
    );
    let email = create_memo(
        clone!(member -> move || member.get().map(|member| member.email).unwrap_or_default()),
    );
    let fixed = create_memo(clone!(member -> move || member.get().and_then(|member| member.fixed)));
    let is_fixed = create_memo(clone!(fixed -> move || fixed.get().is_some()));
    let editable = create_memo(clone!(is_fixed -> move || !is_fixed.get()));
    let locked = is_fixed.clone();
    let fixed_text = create_memo(move || fixed.get().unwrap_or_default());
    let note = create_memo(clone!(member -> move || member.get().and_then(|member| member.note)));
    let has_note = create_memo(clone!(note -> move || note.get().is_some()));
    let note_text = create_memo(move || note.get().unwrap_or_default());
    let access_label = create_memo(clone!(member -> move || {
        member
            .get()
            .map(|member| member.access.label().to_owned())
            .unwrap_or_default()
    }));
    let selected = create_memo(clone!(member -> move || {
        member.get().and_then(|member| GRANTABLE.iter().position(|access| *access == member.access))
    }));
    let removable =
        create_memo(clone!(member -> move || member.get().is_some_and(|member| member.removable)));
    let chosen = member.clone();
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
                        <Show condition={locked}>
                            <List direction=Direction::Horizontal align=Align::Center spacing=4.0>
                                <Icon glyph={ICON_LOCK.to_owned()} text_size=12.0 />
                                <Caption content={fixed_text} />
                            </List>
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
                        <Select
                            selected={selected}
                            label="Access"
                            on_change={move |index: Option<usize>| {
                                let Some(member) = chosen.get_untracked() else {
                                    return;
                                };
                                let access = match index {
                                    Some(index) => GRANTABLE.get(index).copied().unwrap_or(BlockAccess::None),
                                    None => return,
                                };
                                if access != member.access {
                                    share(ShareCommand::SetAccess(member.id, access));
                                }
                            }}
                            options={view! {
                                <ChoiceOption label={BlockAccess::Edit.label()} />
                                <ChoiceOption label={BlockAccess::View.label()} />
                                <ChoiceOption label={BlockAccess::KnowExists.label()} />
                            }}
                        />
                    </Show>
                    <Show condition={removable}>
                        <IconButton
                            glyph={ICON_CLOSE.to_owned()}
                            label="Remove access"
                            size=IconButtonSize::Compact
                            on_click={move || {
                                if let Some(member) = member.get_untracked() {
                                    share(ShareCommand::SetAccess(member.id, BlockAccess::None));
                                }
                            }}
                        />
                    </Show>
                </List>
            </Card>
        </Frame>
    }
}
