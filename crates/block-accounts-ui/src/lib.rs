use beui::icons::{ICON_CHEVRON_DOWN, ICON_CLOUD, ICON_ELLIPSIS, ICON_MONITOR, ICON_PLUS};
use beui::reactive::*;
use beui::styled::*;
use beui::{Align, Direction, Extent, ItemSize, Justify, NodeId, TextAlign};

const COLUMN_WIDTH: f32 = 460.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccountRow {
    pub key: String,
    pub name: String,
    pub email: String,
    pub server: String,
    pub local: bool,
    pub has_last_organization: bool,
    pub current: bool,
    pub companies: Vec<String>,
}

#[component]
pub fn ErrorText(text: Memo<Option<String>>) -> NodeId {
    let shown = create_memo(move || text.get().is_some());
    let content = create_memo(move || text.get().unwrap_or_default());
    view! {
        <List spacing=Space::None>
            <Show condition={shown}>
                <Text style=TextStyle::PARAGRAPH color=TextColor::Danger>{content}</Text>
            </Show>
        </List>
    }
}

#[component]
pub fn Column(children: Children<ListChild>) -> NodeId {
    view! {
        <Scroll>
            <List spacing=Space::None align=Align::Center>
                <Frame
                    width={Extent::at_most(COLUMN_WIDTH)}
                    padding={Padding::symmetric(Space::Xl, Space::Xl3)}
                >
                    <List spacing=Space::Lg>{children}</List>
                </Frame>
            </List>
        </Scroll>
    }
}

#[component]
pub fn AccountsScreen(
    accounts: Memo<Vec<AccountRow>>,
    error: Memo<Option<String>>,
    on_add: ClickCallback,
    on_open: Callback<String>,
    on_choose_organization: Callback<String>,
    on_log_out: Callback<String>,
) -> NodeId {
    let keys = create_memo(move || {
        accounts
            .get()
            .into_iter()
            .map(|account| account.key)
            .collect::<Vec<_>>()
    });
    let empty = create_memo(move || accounts.get().is_empty());
    view! {
        <Column>
            <Text style=TextStyle::TITLE>"Block Editor"</Text>
            <Text style=TextStyle::CAPTION>"Choose an account to continue."</Text>
            <ForEach keys={keys}>|key: String| {
                let account = create_memo(move || {
                    accounts
                        .get()
                        .into_iter()
                        .find(|account| account.key == key)
                        .unwrap_or_default()
                });
                view! {
                    <AccountCard account on_open on_choose_organization on_log_out />
                }
            }</ForEach>
            <Show condition={empty}>
                <Card>
                    <Text style=TextStyle::CAPTION>"No accounts yet. Add one to get started."</Text>
                </Card>
            </Show>
            <Button
                label="Add account"
                glyph=ICON_PLUS
                variant=ButtonVariant::Secondary
                on_click={move || on_add.call()}
            />
            <ErrorText text={error} />
        </Column>
    }
}

#[component]
fn AccountCard(
    account: Memo<AccountRow>,
    on_open: Callback<String>,
    on_choose_organization: Callback<String>,
    on_log_out: Callback<String>,
) -> NodeId {
    let name = create_memo(move || account.get().name);
    let key = create_memo(move || account.get().key);
    let open_hint = create_memo(move || match account.get().has_last_organization {
        true => "Open the last organization used".to_owned(),
        false => "Choose an organization".to_owned(),
    });
    view! {
        <Card>
            <List spacing=Space::Sm>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Text style=TextStyle::HEADING @sizing=ItemSize::Percent(100.0)>{name}</Text>
                    <MenuButton
                        label="Account options"
                        glyph=ICON_ELLIPSIS
                        icon_only=true
                        arrow=false
                        @test_id={format!("account.options.{}", name.get_untracked())}
                        on_select={move |_path: Vec<usize>| {
                            on_log_out.call(key.get_untracked());
                        }}
                        items={view! {
                            <MenuItem
                                label={create_memo(move || if account.get().local { "Close account" } else { "Log out" }.to_owned())}
                            />
                        }}
                    />
                </List>
                <AccountDetails account />
                <List
                    direction=Direction::Horizontal
                    align=Align::Center
                    justify=Justify::End
                    spacing=Space::Xs
                >
                    <Tooltip label={open_hint}>
                        <Button
                            label="Open"
                            variant=ButtonVariant::Primary
                            @test_id={format!("account.open.{}", name.get_untracked())}
                            on_click={move || on_open.call(key.get_untracked())}
                        />
                    </Tooltip>
                    <Button
                        icon_only=true
                        glyph=ICON_CHEVRON_DOWN
                        label="Open a different organization"
                        @test_id={format!("account.choose.{}", name.get_untracked())}
                        on_click={move || on_choose_organization.call(key.get_untracked())}
                    />
                </List>
            </List>
        </Card>
    }
}

#[component]
pub fn AccountDetails(account: Memo<AccountRow>) -> NodeId {
    let email = create_memo(move || account.get().email);
    let server = create_memo(move || account.get().server);
    let glyph = create_memo(move || match account.get().local {
        true => ICON_MONITOR,
        false => ICON_CLOUD,
    });
    let seen_by = create_memo(move || administrators_note(&account.get().companies));
    let seen = create_memo(move || !seen_by.get().is_empty());
    view! {
        <List spacing=Space::Xs2>
            <Show condition={create_memo(move || !email.get().is_empty())}>
                <Text style=TextStyle::CAPTION>{email}</Text>
            </Show>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
                <Text size=TextSize::Sm align=TextAlign::Center>
                    <Icon glyph={glyph} />
                </Text>
                <Text style=TextStyle::CAPTION>{server}</Text>
            </List>
            <Show condition={seen}>
                <Text style=TextStyle::CAPTION>{seen_by}</Text>
            </Show>
        </List>
    }
}

fn administrators_note(companies: &[String]) -> String {
    let named = match companies {
        [] => return String::new(),
        [only] => only.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    let place = match companies.len() {
        1 => "there",
        _ => "in each",
    };
    format!("The administrators of {named} can read your settings and blocks {place}.")
}

#[cfg(test)]
mod tests;
