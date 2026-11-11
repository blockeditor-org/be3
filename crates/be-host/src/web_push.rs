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

/// A block a pressed notification named, waiting for the host's next frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Opening {
    pub(crate) organization: Uuid,
    pub(crate) block: Uuid,
    /// Frames it has waited for the block to be known, which a page opened by a press
    /// has to do while the organization loads.
    pub(crate) waited: u32,
}

/// How many frames an opening waits for its block before it is dropped.
pub(crate) const OPENING_PATIENCE: u32 = 600;

pub(crate) fn requeue(opening: Opening) {
    if opening.waited < OPENING_PATIENCE {
        OPENINGS.with(|openings| {
            openings.borrow_mut().push(Opening {
                waited: opening.waited + 1,
                ..opening
            })
        });
    }
}

thread_local! {
    static OPENINGS: RefCell<Vec<Opening>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn take_openings() -> Vec<Opening> {
    OPENINGS.with(|openings| std::mem::take(&mut *openings.borrow_mut()))
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

/// Opens a push the service worker received: `{ title, body }` for its notification, or
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
    Ok(answer.into())
}

/// A notification was pressed: the host opens its block on the next frame.
#[wasm_bindgen]
pub fn push_open_block(organization: String, block: String) {
    if let (Ok(organization), Ok(block)) =
        (organization.parse::<Uuid>(), block.parse::<Uuid>())
    {
        OPENINGS.with(|openings| {
            openings
                .borrow_mut()
                .push(Opening {
                organization,
                block,
                waited: 0,
            })
        });
        be_host_base::host::wake();
    }
}

/// Signing out: drops this browser's subscription (host.js's `bePush.unsubscribe`) and asks
/// the server to forget its endpoint. The server's request may not get through before the
/// account's connection closes; the dropped subscription makes the push service answer 404 or
/// 410 to the next push, and the server drops the endpoint then.
pub(crate) fn forget_this_device() {
    wasm_bindgen_futures::spawn_local(async {
        let Ok(push) = js_sys::Reflect::get(&js_sys::global(), &"bePush".into()) else {
            return;
        };
        let Ok(unsubscribe) = js_sys::Reflect::get(&push, &"unsubscribe".into())
            .and_then(|function| function.dyn_into::<js_sys::Function>())
        else {
            return;
        };
        let Ok(promise) = unsubscribe.call0(&push) else {
            return;
        };
        let Ok(address) = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise)).await
        else {
            return;
        };
        if let Some(address) = address.as_string() {
            let _ = push_unregister(address).await;
        }
    });
}
