//! What the page's push notifications (web/push.js, web/sw.js) call into the host:
//! subscribing registers an endpoint with the server, the service worker hands a push
//! to be opened and named, and pressing a notification asks the host to open its block.
//! plans/notifications.md has the design.

use std::cell::RefCell;

use be_block_protocol::{PushEndpoint, PushMessage, PushService};
use tokio::sync::oneshot;
use uuid::Uuid;
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
use web_sys::js_sys;

/// A block a pressed notification named, waiting until the host can open it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Opening {
    pub(crate) organization: Uuid,
    pub(crate) block: Uuid,
}

thread_local! {
    static OPENINGS: RefCell<Vec<Opening>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn has_openings() -> bool {
    OPENINGS.with(|openings| !openings.borrow().is_empty())
}

pub(crate) fn take_openings() -> Vec<Opening> {
    OPENINGS.with(|openings| std::mem::take(&mut *openings.borrow_mut()))
}

/// Holds an opening that could not happen yet: the host tries again when its organization or
/// graph changes.
pub(crate) fn hold(opening: Opening) {
    OPENINGS.with(|openings| openings.borrow_mut().push(opening));
}

async fn answered<T>(
    ask: impl FnOnce(Box<dyn FnOnce(Result<T, String>) + Send>),
) -> Result<T, JsValue>
where
    T: Send + 'static,
{
    let (reply, answer) = oneshot::channel();
    ask(Box::new(move |result| {
        let _ = reply.send(result);
    }));
    answer
        .await
        .map_err(|_| JsValue::from_str("be-host stopped"))?
        .map_err(|error| JsValue::from_str(&error))
}

/// The server's VAPID public key, which the browser subscribes with.
#[wasm_bindgen]
pub async fn push_server_key() -> Result<Vec<u8>, JsValue> {
    answered(be_host_stack::push_key).await
}

/// Registers the subscription the browser made.
#[wasm_bindgen]
pub async fn push_register(address: String, p256dh: Vec<u8>, auth: Vec<u8>) -> Result<(), JsValue> {
    let endpoint = PushEndpoint {
        service: PushService::WebPush,
        address,
        p256dh,
        auth,
    };
    answered(|reply| be_host_stack::register_push_endpoint(endpoint, reply)).await
}

#[wasm_bindgen]
pub async fn push_unregister(address: String) -> Result<(), JsValue> {
    answered(|reply| be_host_stack::unregister_push_endpoint(address, reply)).await
}

/// Opens a push the service worker received: `{ title, body, block, organization }` for its
/// notification (the block and organization the push was sealed about), or
/// null when this device cannot open it (no key yet, another block's, too old).
#[wasm_bindgen]
pub async fn push_open(bytes: Vec<u8>) -> Result<JsValue, JsValue> {
    let Some(message) = PushMessage::decode(&bytes) else {
        return Ok(JsValue::NULL);
    };
    let now = js_sys::Date::now() as i64;
    let Ok(opened) = answered(|reply| be_host_stack::open_push(message, now, reply)).await else {
        return Ok(JsValue::NULL);
    };
    // The sender and the block are named from this device's own contacts and graph,
    // never from what the sender wrote.
    let sender = be_host_stack::contacts()
        .into_iter()
        .find(|contact| contact.account == opened.sender)
        .map(|contact| contact.display_name);
    let block = be_host_stack::node(opened.block).and_then(|node| node.metadata.name());
    let title = block.unwrap_or_else(|| "New message".to_owned());
    let body = match sender {
        Some(sender) => format!("{sender}: {}", opened.text),
        None => opened.text,
    };
    let answer = js_sys::Object::new();
    js_sys::Reflect::set(&answer, &"title".into(), &title.into())?;
    js_sys::Reflect::set(&answer, &"body".into(), &body.into())?;
    js_sys::Reflect::set(&answer, &"block".into(), &opened.block.to_string().into())?;
    js_sys::Reflect::set(
        &answer,
        &"organization".into(),
        &opened.organization.to_string().into(),
    )?;
    Ok(answer.into())
}

/// A notification was pressed: the host opens its block on the next frame.
#[wasm_bindgen]
pub fn push_open_block(organization: String, block: String) {
    if let (Ok(organization), Ok(block)) = (organization.parse::<Uuid>(), block.parse::<Uuid>()) {
        hold(Opening {
            organization,
            block,
        });
        be_host_base::host::wake();
    }
}

/// Signing out: asks the server to forget this browser's endpoint for the account, and drops
/// the browser's subscription too when `unsubscribe` (no saved account is left to use it). The
/// server's request may not get through before the account's connection closes; a dropped
/// subscription makes the push service answer 404 or 410 to the next push, and the server drops
/// the endpoint then. Another account removed while this one is open keeps its endpoint until
/// then.
pub(crate) fn forget_this_device(unsubscribe: bool) {
    wasm_bindgen_futures::spawn_local(async move {
        if let Some(address) = call("address")
            .await
            .ok()
            .and_then(|address| address.as_string())
        {
            let _ = push_unregister(address).await;
        }
        if unsubscribe && let Err(error) = call("unsubscribe").await {
            web_sys::console::warn_1(&JsValue::from_str(&format!(
                "be-host: the browser's push subscription was not dropped: {error}"
            )));
        }
    });
}

fn push() -> Option<JsValue> {
    js_sys::Reflect::get(&js_sys::global(), &"bePush".into())
        .ok()
        .filter(|push| !push.is_undefined())
}

/// Calls `bePush.method()` (host.js) and awaits what it returns.
async fn call(method: &str) -> Result<JsValue, String> {
    let push = push().ok_or("This browser cannot show notifications.")?;
    let function = js_sys::Reflect::get(&push, &method.into())
        .and_then(|function| function.dyn_into::<js_sys::Function>())
        .map_err(|_| format!("bePush.{method} is missing"))?;
    let promise = function.call0(&push).map_err(|error| error_text(&error))?;
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&promise))
        .await
        .map_err(|error| error_text(&error))
}

fn error_text(error: &JsValue) -> String {
    match error.clone().dyn_into::<js_sys::Error>() {
        Ok(error) => String::from(error.message()),
        Err(error) => error.as_string().unwrap_or_else(|| "failed".to_owned()),
    }
}

/// Reads this browser's state (`bePush.state`) into the host's.
async fn refresh() {
    use be_host_base::push_device::{PushDevice, set_state};
    let state = match call("state").await.ok().and_then(|state| state.as_string()) {
        Some(state) => match state.as_str() {
            "on" => PushDevice::On,
            "off" => PushDevice::Off,
            "denied" => PushDevice::Denied,
            _ => PushDevice::Unsupported,
        },
        None => PushDevice::Unsupported,
    };
    set_state(state);
}

/// Lets apps turn pushes on this browser on and off (`host-services.set-push-device`), and
/// reads what it does now.
pub(crate) fn install() {
    be_host_base::push_device::install(std::rc::Rc::new(|on, reply| {
        wasm_bindgen_futures::spawn_local(async move {
            let done = call(if on { "enable" } else { "disable" }).await;
            refresh().await;
            reply(done.map(|_| ()));
        });
    }));
    wasm_bindgen_futures::spawn_local(refresh());
}

/// Tells the server about this browser's subscription for the open organization
/// (`bePush.sync`): the push service may have replaced it, and an endpoint is registered per
/// organization.
pub(crate) fn sync() {
    wasm_bindgen_futures::spawn_local(async {
        let _ = call("sync").await;
    });
}
