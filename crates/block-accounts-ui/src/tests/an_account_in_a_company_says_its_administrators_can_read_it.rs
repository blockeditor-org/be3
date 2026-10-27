use super::*;
use beui::native::build;

#[test]
fn an_account_in_a_company_says_its_administrators_can_read_it() {
    let document = build(|| {
        let accounts = create_memo(|| {
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
        });
        let error = create_memo(|| None);
        view! {
            <beui::styled::Root>
                <AccountsScreen
                    accounts
                    error
                    on_add={|| {}}
                    on_open={|_: String| {}}
                    on_choose_organization={|_: String| {}}
                    on_log_out={|_: String| {}}
                />
            </beui::styled::Root>
        }
    });
    painting_of(
        "an_account_in_a_company_says_its_administrators_can_read_it",
        document,
    );
}
