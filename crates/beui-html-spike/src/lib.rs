use beui::reactive::*;
use beui::styled::*;
use beui::{ItemSize, primitives};
use block_accounts_ui::{AccountRow, AccountsScreen};

#[cfg(target_arch = "wasm32")]
mod web;

fn fixture_accounts() -> Vec<AccountRow> {
    vec![
        AccountRow {
            key: "work".into(),
            name: "Ada".into(),
            email: "ada@example.com".into(),
            server: "https://api.example.com".into(),
            has_last_organization: true,
            companies: vec!["Analytical Engines".into()],
            ..Default::default()
        },
        AccountRow {
            key: "home".into(),
            name: "Ada at home".into(),
            email: "ada@home.example".into(),
            server: "https://api.example.com".into(),
            ..Default::default()
        },
    ]
}

fn added(number: usize) -> AccountRow {
    AccountRow {
        key: format!("added-{number}"),
        name: format!("Added account {number}"),
        email: format!("added{number}@example.com"),
        server: "https://api.example.com".into(),
        ..Default::default()
    }
}

#[component]
pub fn Spike() -> primitives::Root {
    let accounts = create_signal(fixture_accounts());
    let status = create_signal(String::new());
    let listed = create_memo(move || accounts.get());
    let error = create_memo(|| None);
    let name_of = move |key: &str| {
        accounts
            .get_untracked()
            .into_iter()
            .find(|account| account.key == key)
            .map(|account| account.name)
            .unwrap_or_default()
    };
    view! {
        <Root>
            <List spacing=Space::None>
                <AccountsScreen
                    @sizing=ItemSize::Percent(100.0)
                    accounts={listed}
                    error
                    on_add={move || {
                        let number = accounts.update(|accounts| {
                            let number = accounts.len() + 1;
                            accounts.push(added(number));
                            number
                        });
                        status.set(format!("Added account {number}"));
                    }}
                    on_open={move |key: String| status.set(format!("Opened {}", name_of(&key)))}
                    on_choose_organization={move |key: String| {
                        status.set(format!("Choosing an organization for {}", name_of(&key)))
                    }}
                    on_log_out={move |key: String| {
                        status.set(format!("Logged out of {}", name_of(&key)));
                        accounts.update(|accounts| accounts.retain(|account| account.key != key));
                    }}
                />
                <Frame padding={Padding::all(Space::Md)}>
                    <Text style=TextStyle::CAPTION @test_id="spike.status">{status}</Text>
                </Frame>
            </List>
        </Root>
    }
}

pub fn markup() -> String {
    beui::html::render_to_string(|| {
        view! {
            <Spike />
        }
    })
}

pub fn page() -> String {
    let markup = markup();
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Accounts on beui's HTML core</title>
<style id="{}">{}</style>
<script type="importmap">{{ "imports": {{ "wasi_snapshot_preview1": "./wasi.js", "wasi": "./threads.js" }} }}</script>
<script type="module" src="./start.js"></script>
</head>
<body><div id="app">{markup}</div></body>
</html>
"#,
        beui::html::STYLES_ID,
        beui::html::stylesheet(),
    )
}

#[cfg(test)]
mod tests;
