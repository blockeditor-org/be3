//! The sign-in page: how to make a boat.dev key, and the fields to paste it and its expiry
//! into, checked with `GET /me` before it is kept.

use be_sdk_beui::beui::datetime::{Date, DateTime, Time};
use be_sdk_beui::beui::icons::{ICON_EXTERNAL_LINK, ICON_KEY_ROUND};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{Align, Direction, NodeId};
use be_type_boat::BoatKey;

use super::state::State;
use super::ui::{SECTION_SPACING, SPACING};
use crate::api::{self, ACTIONS, API_KEYS, Account, BoatError, KEY_DAYS};
use crate::time::DAY;

#[component]
pub(super) fn SignIn(state: State) -> NodeId {
    let editor = state.editor;
    let read_only = editor.read_only();
    let organization =
        create_memo(move || state.filters.with(|filters| filters.organization.clone()));
    let organization_draft = create_signal(organization.get_untracked());
    create_effect(move || organization_draft.set(organization.get()));
    let secret = create_signal(String::new());
    let today = Date::today();
    let expiry = create_signal(Some(DateTime::new(
        today.add_days(KEY_DAYS),
        Time::MIDNIGHT,
    )));
    let checking = create_signal(false);
    let problem = create_signal(None::<String>);
    let reconnecting = state.reconnecting;
    let heading = create_memo(move || match state.key.with(Option::is_some) {
        true => "Reconnect to boat.dev".to_owned(),
        false => "Connect to boat.dev".to_owned(),
    });
    let whose = create_memo(move || {
        match organization.get() {
        organization if organization.is_empty() => {
            "Make the key in your own boat.dev account, or name an organization below to show its sandboxes instead.".to_owned()
        }
        organization => format!("Make the key in the {organization} organization: an organization's key sees only that organization."),
    }
    });
    let actions = ACTIONS.join(", ");
    let command = api::create_command();
    let unchecked = create_memo(move || {
        checking.get()
            || secret.with(|secret| secret.trim().is_empty())
            || expiry.with(Option::is_none)
    });
    let connect = move || {
        let key = secret.get_untracked().trim().to_owned();
        let Some(expires) = expiry.get_untracked() else {
            return;
        };
        if key.is_empty() || checking.get_untracked() {
            return;
        }
        checking.set(true);
        problem.set(None);
        let account = Account {
            key: key.clone(),
            organization: organization.get_untracked(),
        };
        let expires = expires.date.days() * DAY;
        editor.then(api::me(&editor.host(), &account), move |answer| {
            checking.set(false);
            match answer {
                Ok(login) => {
                    secret.set(String::new());
                    state.keep_key(BoatKey {
                        secret: key,
                        expires,
                        login,
                    });
                }
                Err(BoatError::Reconnect { expired: true }) => {
                    problem.set(Some("boat.dev says this key has expired.".to_owned()))
                }
                Err(BoatError::Reconnect { expired: false }) => problem.set(Some(
                    "boat.dev doesn't accept this key. Check that all of it was pasted.".to_owned(),
                )),
                Err(error) => problem.set(Some(error.to_string())),
            }
        });
    };
    let open_keys = move || {
        editor.then(editor.host().open_url(API_KEYS), |_| {});
    };
    let problem_text = create_memo(move || problem.get().unwrap_or_default());
    let has_problem = create_memo(move || problem.with(Option::is_some));
    let replacing = create_memo(move || reconnecting.get() && state.key.with(Option::is_some));
    view! {
        <List spacing=SECTION_SPACING @test_id="boat.sign-in">
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Text style=TextStyle::HEADING>
                        <Icon glyph=ICON_KEY_ROUND />
                    </Text>
                    <Text style=TextStyle::HEADING>{heading}</Text>
                </List>
                <Text style=TextStyle::PARAGRAPH>
                    "This block lists your boat.dev sandboxes and starts and stops them with an API \
                     key of your own. The key is kept in your settings, where only you can read it."
                </Text>
            </List>
            <Card>
                <List spacing=SECTION_SPACING>
                    <Step number=1 title="Open boat.dev's API Keys tab">
                        <List spacing=SPACING>
                            <Text style=TextStyle::CAPTION>{whose}</Text>
                            <List
                                direction=Direction::Horizontal
                                align=Align::Center
                                spacing=SPACING
                            >
                                <Button
                                    label="Open API Keys"
                                    glyph=ICON_EXTERNAL_LINK
                                    @test_id="boat.sign-in.open"
                                    on_click={move || open_keys()}
                                />
                            </List>
                            <Text style=TextStyle::CAPTION>"Organization, by name or id"</Text>
                            <TextInput
                                value={organization_draft}
                                label="Organization"
                                placeholder="Your own account"
                                disabled={read_only}
                                @test_id="boat.sign-in.organization"
                                on_submit={move |value: String| state.set_organization(value)}
                                on_focus_change={move |focused: bool| {
                                    if !focused && organization_draft.get_untracked() != organization.get_untracked() {
                                        state.set_organization(organization_draft.get_untracked());
                                    }
                                }}
                            />
                        </List>
                    </Step>
                    <Step number=2 title="Make a key with these actions">
                        <List spacing=SPACING>
                            <Text style=TextStyle::CODE @test_id="boat.sign-in.actions">
                                {actions}
                            </Text>
                            <Text style=TextStyle::CAPTION>
                                {format!("Set it to expire in {KEY_DAYS} days, the longest boat.dev allows. Or make it with boat's command line:")}
                            </Text>
                            <TextInput
                                value={create_signal(command)}
                                label="Command"
                                copyable=true
                                @test_id="boat.sign-in.command"
                            />
                        </List>
                    </Step>
                    <Step number=3 title="Paste it here">
                        <List spacing=SPACING>
                            <TextInput
                                value={secret}
                                label="API key"
                                placeholder="boat_..."
                                kind=InputKind::Password
                                @test_id="boat.sign-in.key"
                                on_submit={move |_| connect()}
                            />
                            <DateTimeField
                                value={expiry}
                                parts=DateTimeParts::Date
                                label="Expires on"
                                min={Some(today)}
                                today={Some(today)}
                            />
                            <Text style=TextStyle::CAPTION>
                                "boat.dev doesn't say when a key expires, so this is when the block reminds you to make a new one."
                            </Text>
                        </List>
                    </Step>
                    <Show condition={has_problem}>
                        <Text color=TextColor::Danger @test_id="boat.sign-in.problem">
                            {problem_text}
                        </Text>
                    </Show>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Button
                            label="Connect"
                            variant=ButtonVariant::Primary
                            disabled={unchecked}
                            busy={checking}
                            @test_id="boat.sign-in.connect"
                            on_click={move || connect()}
                        />
                        <Show condition={replacing}>
                            <Button
                                label="Cancel"
                                @test_id="boat.sign-in.cancel"
                                on_click={move || reconnecting.set(false)}
                            />
                        </Show>
                    </List>
                </List>
            </Card>
        </List>
    }
}

#[component]
fn Step(number: u32, title: &'static str, children: Child) -> NodeId {
    view! {
        <List spacing=SPACING>
            <Text weight=Weight::Bold>{format!("{number}. {title}")}</Text>
            {children}
        </List>
    }
}
