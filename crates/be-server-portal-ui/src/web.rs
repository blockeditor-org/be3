use be_server_portal::{PAGE_ID, Page, PasskeyOffer, ROOT_ID};
use beui::reactive::*;
use js_sys::{Function, JSON, Object, Promise, Reflect};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use crate::PortalPage;

#[wasm_bindgen]
pub fn start(page: String) -> Result<(), JsValue> {
    be_wasm_threads::initialize_page();
    let page = Page::from_json(&page)
        .map_err(|error| JsValue::from_str(&format!("{PAGE_ID}: {error}")))?;
    beui::html::hydrate(ROOT_ID, move || {
        view! {
            <PortalPage page />
        }
    })
    .map_err(|error| JsValue::from_str(&error))
}

fn get(target: &JsValue, name: &str) -> Result<JsValue, JsValue> {
    Reflect::get(target, &JsValue::from_str(name))
}

/// The browser's `PublicKeyCredential`, if it has WebAuthn's JSON methods.
fn credential_class() -> Option<JsValue> {
    let window: JsValue = web_sys::window()?.into();
    let class = get(&window, "PublicKeyCredential").ok()?;
    let prototype = get(&class, "prototype").ok()?;
    let usable = [
        "parseRequestOptionsFromJSON",
        "parseCreationOptionsFromJSON",
    ]
    .iter()
    .all(|name| get(&class, name).is_ok_and(|method| method.is_function()))
        && get(&prototype, "toJSON").is_ok_and(|method| method.is_function());
    usable.then_some(class)
}

/// Hides a passkey button, once the page hydrated, in a browser that can't run its ceremony.
pub(crate) fn check_passkeys(usable: Signal<bool>) {
    wasm_bindgen_futures::spawn_local(async move {
        if credential_class().is_none() {
            usable.set(false);
        }
    });
}

/// Begins the browser's ceremony at once, in the click that asked for it, and posts its answer.
pub(crate) fn use_passkey(offer: &PasskeyOffer, csrf: &str, code: &str, problem: Signal<String>) {
    problem.set(String::new());
    let begun = begin(offer);
    let fields = [("csrf", csrf.to_owned()), ("code", code.to_owned())];
    let action = offer.step.action();
    let begun = match begun {
        Ok(begun) => begun,
        Err(error) => return problem.set(failure(&error)),
    };
    wasm_bindgen_futures::spawn_local(async move {
        let answered = async {
            let credential = JsFuture::from(begun).await?;
            let method: Function = get(&credential, "toJSON")?.dyn_into()?;
            let json = JSON::stringify(&method.call0(&credential)?)?;
            Ok::<String, JsValue>(String::from(json))
        };
        match answered.await {
            Ok(json) => {
                let mut fields = fields.to_vec();
                fields.push(("credential", json));
                if let Err(error) = post(action, &fields) {
                    problem.set(failure(&error));
                }
            }
            Err(error) => problem.set(failure(&error)),
        }
    });
}

fn begin(offer: &PasskeyOffer) -> Result<Promise, JsValue> {
    let class = credential_class().ok_or_else(|| JsValue::from_str("unsupported"))?;
    let parse = match offer.step.creates() {
        true => "parseCreationOptionsFromJSON",
        false => "parseRequestOptionsFromJSON",
    };
    let parse: Function = get(&class, parse)?.dyn_into()?;
    let options = parse.call1(&class, &JSON::parse(&offer.options)?)?;
    let request = Object::new();
    Reflect::set(&request, &JsValue::from_str("publicKey"), &options)?;
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let credentials = window.navigator().credentials();
    match offer.step.creates() {
        true => credentials.create_with_options(request.unchecked_ref()),
        false => credentials.get_with_options(request.unchecked_ref()),
    }
}

/// Posts `fields` to `action` as the page's own forms do, so the answer meets the same checks.
fn post(action: &str, fields: &[(&str, String)]) -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let form: web_sys::HtmlFormElement = document.create_element("form")?.dyn_into()?;
    form.set_method("post");
    form.set_action(action);
    for (name, value) in fields {
        let input: web_sys::HtmlInputElement = document.create_element("input")?.dyn_into()?;
        input.set_type("hidden");
        input.set_name(name);
        input.set_value(value);
        form.append_child(&input)?;
    }
    document
        .body()
        .ok_or_else(|| JsValue::from_str("no body"))?
        .append_child(&form)?;
    form.submit()
}

/// What the page says when the browser's ceremony fails.
fn failure(error: &JsValue) -> String {
    let name = get(error, "name")
        .ok()
        .and_then(|name| name.as_string())
        .unwrap_or_default();
    match name.as_str() {
        "NotAllowedError" | "AbortError" => {
            "No passkey was used. Try again, or sign in another way.".to_owned()
        }
        "InvalidStateError" => "This device already has a passkey for your account.".to_owned(),
        _ => "This browser could not use a passkey here. Sign in another way.".to_owned(),
    }
}
