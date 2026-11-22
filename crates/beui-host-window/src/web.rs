//! A program in a browser: what [`main!`](crate::main) exports to the page.

use std::cell::RefCell;
use std::error::Error;
use std::fmt;

#[doc(hidden)]
pub use wasm_bindgen;
#[doc(hidden)]
pub use wasm_bindgen_futures;

use wasm_bindgen::JsValue;

thread_local! {
    static ELEMENT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// An error the page tells apart by its name, as be-host's page tells saved data it could
/// not read from drawing that failed: `beui_start` rejects with a JavaScript `Error` of that
/// name. Any other error rejects as an `Error` named `Error`.
#[derive(Debug)]
pub struct NamedError {
    pub name: &'static str,
    pub message: String,
}

impl NamedError {
    pub fn new(name: &'static str, message: impl Into<String>) -> Self {
        Self {
            name,
            message: message.into(),
        }
    }
}

impl fmt::Display for NamedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for NamedError {}

/// The id of the element the page asked the program to show in.
pub(crate) fn element() -> String {
    ELEMENT.with_borrow(Clone::clone)
}

#[doc(hidden)]
pub async fn start<E: Into<Box<dyn Error>>>(
    element_id: String,
    main: impl Future<Output = Result<(), E>>,
) -> Result<(), JsValue> {
    be_wasm_threads::initialize_page();
    ELEMENT.set(element_id);
    main.await.map_err(|error| {
        let error = error.into();
        let thrown = web_sys::js_sys::Error::new(&error.to_string());
        if let Some(named) = error.downcast_ref::<NamedError>() {
            thrown.set_name(named.name);
        }
        JsValue::from(thrown)
    })
}

#[cfg(not(beui_html))]
pub(crate) async fn run_program(
    options: beui_host::native::RunOptions,
    program: impl beui_host::native::Program + 'static,
) -> Result<(), Box<dyn Error>> {
    let mut layers: Vec<Box<dyn beui_host::native::Layer>> = Vec::new();
    if let Some(inbox) = automation()? {
        layers.push(Box::new(beui_host::native::drive::Drive::new(inbox)));
    }
    beui_host::native::run_web(&element(), options, layers, program).await
}

#[cfg(not(beui_html))]
thread_local! {
    static AUTOMATION: RefCell<Option<beui_host::native::drive::Inbox>> =
        const { RefCell::new(None) };
}

/// The drive layer's inbox, for a page opened with `?automation`.
#[cfg(not(beui_html))]
fn automation() -> Result<Option<beui_host::native::drive::Inbox>, String> {
    let location = web_sys::window()
        .map(|window| window.location())
        .ok_or("no browser window is available")?;
    let address = location
        .href()
        .map_err(|error| format!("the page's address cannot be read: {error:?}"))?;
    let address = web_sys::Url::new(&address)
        .map_err(|error| format!("the page's address cannot be read: {error:?}"))?;
    if !address.search_params().has("automation") {
        return Ok(None);
    }
    Ok(Some(AUTOMATION.with(|automation| {
        automation.borrow_mut().get_or_insert_default().clone()
    })))
}

/// What drive waits for an answer when it names no timeout, as its own default.
#[cfg(not(beui_html))]
const AUTOMATE_TIMEOUT_MILLISECONDS: i32 = 30_000;

#[doc(hidden)]
#[cfg(not(beui_html))]
pub async fn automate(
    words: Vec<String>,
    timeout_milliseconds: Option<i32>,
) -> Result<String, JsValue> {
    use beui_host::native::drive;
    use wasm_bindgen::JsCast;
    let timeout_milliseconds = timeout_milliseconds.unwrap_or(AUTOMATE_TIMEOUT_MILLISECONDS);
    let inbox = automation()?.ok_or("the page was not opened with ?automation")?;
    let answered = drive::ask(&inbox, words);
    let canceller = answered.canceller();
    let late = wasm_bindgen::closure::Closure::once_into_js(move || {
        canceller.cancel(drive::timed_out(
            std::time::Duration::from_millis(u64::try_from(timeout_milliseconds).unwrap_or(0)),
            &inbox,
        ));
    });
    web_sys::window()
        .ok_or("no browser window is available")?
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            late.unchecked_ref(),
            timeout_milliseconds,
        )?;
    match answered.await {
        drive::Reply::Text(text) => Ok(text),
        drive::Reply::Error(error) => Err(error.into()),
        drive::Reply::Image { .. } => {
            Err("the browser takes screenshots itself: use drive shot".into())
        }
        drive::Reply::Painting(_) => {
            Err("a browser does not answer paint; run the program natively".into())
        }
    }
}

/// The HTML core answers drive's commands itself, from the page, with the timeouts its words
/// name.
#[doc(hidden)]
#[cfg(beui_html)]
pub async fn automate(
    words: Vec<String>,
    _timeout_milliseconds: Option<i32>,
) -> Result<String, JsValue> {
    beui::html::automate(words)
        .await
        .map_err(|error| JsValue::from_str(&error))
}
