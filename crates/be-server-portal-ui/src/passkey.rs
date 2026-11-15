//! The button that runs a passkey ceremony a page offers (`Page::passkey`). be-server renders
//! it with the page; once the wasm runs, a click calls `navigator.credentials` with the page's
//! options and posts the answer to the step's action in a form of its own, with the page's
//! form token, whose ceremony the server spends (be-server's `passkeys.rs`). A browser
//! without WebAuthn's JSON methods hides it.

use be_server_portal::PasskeyOffer;
use beui::NodeId;
use beui::icons::ICON_KEY_ROUND;
use beui::reactive::*;
use beui::styled::*;

#[component]
pub fn PasskeyButton(
    offer: PasskeyOffer,
    csrf: String,
    code: String,
    label: String,
    #[prop(default = ButtonVariant::Secondary)] variant: ButtonVariant,
) -> NodeId {
    let usable = create_signal(true);
    let problem = create_signal(String::new());
    #[cfg(target_arch = "wasm32")]
    crate::web::check_passkeys(usable);
    let on_click = Callback::new(move |()| {
        #[cfg(target_arch = "wasm32")]
        crate::web::use_passkey(&offer, &csrf, &code, problem);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (&offer, &csrf, &code);
    });
    let shown = create_memo(move || problem.get());
    let label = crate::form::fixed(label);
    view! {
        <Show value={move || usable.get()}>
            <List spacing=Space::Xs>
                <Button label variant glyph={Some(ICON_KEY_ROUND)} wide=true on_click />
                <Show value={move || !shown.with(String::is_empty)}>
                    <Text size=TextSize::Sm color=TextColor::Danger>{shown}</Text>
                </Show>
            </List>
        </Show>
    }
}
