use super::*;
use paint_snapshot::BrowserSnapshot;
use uuid::Uuid;

fn page(path: &str) -> Page {
    Page {
        path: path.into(),
        code: String::new(),
        error: String::new(),
        csrf: "snapshot-form-token".into(),
        profile: None,
        sessions: Vec::new(),
        fields: HashMap::new(),
        mail_token: "snapshot-mail-token".into(),
        allow_login: true,
        allow_registration: true,
        code_error: String::new(),
        saved: false,
    }
}

fn signed_in(path: &str) -> Page {
    let mut data = page(path);
    data.profile = Some(Profile {
        account: Uuid::nil(),
        email: "alex@example.com".into(),
        display_name: "Alex".into(),
    });
    data
}

fn painting(data: Page, size: [u32; 2]) -> BrowserSnapshot {
    BrowserSnapshot::new(
        size,
        &render(|| {
            view! {
                <PageView data={data} />
            }
        }),
    )
}

mod the_portal_captures_account_management_and_approval;
mod the_portal_captures_errors_and_server_policy;
mod the_portal_captures_mail_and_completion_pages;
mod the_portal_forms_are_captured_on_desktop_and_phone;
