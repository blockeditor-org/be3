use super::*;

#[test]
fn an_account_in_a_company_says_its_administrators_can_read_it() {
    let document = build(|| {
        let view = AppViewStore::new(crate::ui::AppView {
            accounts: vec![
                crate::ui::AccountRow {
                    key: "work".into(),
                    name: "Ada".into(),
                    email: "ada@example.com".into(),
                    server: "https://api.example.com".into(),
                    has_last_organization: true,
                    companies: vec!["Analytical Engines".into()],
                    ..Default::default()
                },
                crate::ui::AccountRow {
                    key: "home".into(),
                    name: "Ada at home".into(),
                    email: "ada@home.example".into(),
                    server: "https://api.example.com".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        });
        view! {
            <beui::styled::Root>
                <AccountsScreen view />
            </beui::styled::Root>
        }
    });
    painting_of(
        "an_account_in_a_company_says_its_administrators_can_read_it",
        document,
        false,
    );
}
