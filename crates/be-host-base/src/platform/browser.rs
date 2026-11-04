pub fn open_browser(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("The browser link must use HTTP or HTTPS.".into());
    }
    open(url)
}

#[cfg(target_arch = "wasm32")]
fn open(url: &str) -> Result<(), String> {
    web_sys::window()
        .ok_or("No browser window is available")?
        .open_with_url_and_target(url, "_blank")
        .map_err(|_| "Could not open a browser tab")?
        .ok_or("The browser blocked the new tab. Use the link and code below.")?;
    Ok(())
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn open(url: &str) -> Result<(), String> {
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
        .map_err(|error| {
            format!("Could not open a browser: {error}. Use the link and code below.")
        })?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(target_os = "android")]
fn open(url: &str) -> Result<(), String> {
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
        format!("Could not open a browser: {error}. Use the link and code below.")
    })
}
