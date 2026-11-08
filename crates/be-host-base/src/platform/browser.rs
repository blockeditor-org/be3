/// Why a link did not open.
enum Unopened {
    /// The browser blocked the tab: it was opened too long after the press that led to it.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    Blocked,
    Failed(String),
}

/// Opens the sign-in link, which the account screen also shows with its code.
pub fn open_browser(url: &str) -> Result<(), String> {
    opening(url).map_err(|unopened| match unopened {
        Unopened::Blocked => "The browser blocked the new tab. Use the link and code below.".into(),
        Unopened::Failed(error) => format!("{error} Use the link and code below."),
    })
}

/// Opens a link an app asked for (`network.open-url`); where the browser blocks the tab, it
/// shows a notice whose own press opens it.
pub fn open_link(url: &str) -> Result<(), String> {
    match opening(url) {
        Ok(()) => Ok(()),
        Err(Unopened::Blocked) => {
            crate::notices::offer_link(url.to_owned());
            Ok(())
        }
        Err(Unopened::Failed(error)) => Err(error),
    }
}

fn opening(url: &str) -> Result<(), Unopened> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(Unopened::Failed(
            "The link must use HTTP or HTTPS.".to_owned(),
        ));
    }
    open(url)
}

#[cfg(target_arch = "wasm32")]
fn open(url: &str) -> Result<(), Unopened> {
    let failed = |error: &str| Unopened::Failed(error.to_owned());
    web_sys::window()
        .ok_or_else(|| failed("No browser window is available."))?
        .open_with_url_and_target(url, "_blank")
        .map_err(|_| failed("Could not open a browser tab."))?
        .ok_or(Unopened::Blocked)?;
    Ok(())
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn open(url: &str) -> Result<(), Unopened> {
    use std::process::{Command, Stdio};
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler");
        command
    };
    #[cfg(not(any(target_os = "macos", windows)))]
    let mut command = Command::new("xdg-open");
    let mut child = command
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| Unopened::Failed(format!("Could not open a browser: {error}.")))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(target_os = "android")]
fn open(url: &str) -> Result<(), Unopened> {
    use jni::{
        jni_sig, jni_str,
        objects::{JObject, JValue},
        vm::JavaVM,
    };
    let context = ndk_context::android_context();
    // unsafe(ffi)
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
    vm.attach_current_thread_for_scope(|env| {
        // unsafe(ffi)
        let activity = unsafe { JObject::from_raw(env, context.context().cast()) };
        let value = env.new_string(url)?;
        let uri = env
            .call_static_method(
                jni_str!("android/net/Uri"),
                jni_str!("parse"),
                jni_sig!("(Ljava/lang/String;)Landroid/net/Uri;"),
                &[JValue::Object(&value)],
            )?
            .l()?;
        let action = env.new_string("android.intent.action.VIEW")?;
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!("(Ljava/lang/String;Landroid/net/Uri;)V"),
            &[JValue::Object(&action), JValue::Object(&uri)],
        )?;
        env.call_method(
            &activity,
            jni_str!("startActivity"),
            jni_sig!("(Landroid/content/Intent;)V"),
            &[JValue::Object(&intent)],
        )?;
        Ok(())
    })
    .map_err(|error: jni::errors::Error| {
        Unopened::Failed(format!("Could not open a browser: {error}."))
    })
}
