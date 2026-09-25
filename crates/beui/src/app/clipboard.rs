#[derive(Default)]
pub(super) struct Clipboard {
    #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
    inner: Option<arboard::Clipboard>,
}

impl Clipboard {
    pub(super) fn new() -> Self {
        Self {
            #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
            inner: arboard::Clipboard::new().ok(),
        }
    }

    pub(super) fn get(&mut self) -> Option<String> {
        #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
        {
            self.inner.as_mut()?.get_text().ok()
        }
        #[cfg(target_os = "android")]
        {
            android::get().ok().flatten()
        }
        #[cfg(any(target_os = "ios", target_arch = "wasm32"))]
        {
            None
        }
    }

    pub(super) fn set(&mut self, text: String) {
        #[cfg(not(any(target_os = "android", target_os = "ios", target_arch = "wasm32")))]
        if let Some(clipboard) = &mut self.inner {
            let _ = clipboard.set_text(text);
        }
        #[cfg(target_os = "android")]
        let _ = android::set(&text);
        #[cfg(any(target_os = "ios", target_arch = "wasm32"))]
        let _ = text;
    }
}

#[cfg(target_os = "android")]
mod android {
    use jni::errors::Error;
    use jni::objects::{JObject, JString, JValue};
    use jni::vm::JavaVM;
    use jni::{Env, jni_sig, jni_str};

    fn with_manager<T>(
        run: impl FnOnce(&mut Env<'_>, &JObject<'_>, &JObject<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let context = ndk_context::android_context();
        let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
        vm.attach_current_thread_for_scope(|env| {
            let activity = unsafe { JObject::from_raw(env, context.context().cast()) };
            let service = env.new_string("clipboard")?;
            let manager = env
                .call_method(
                    &activity,
                    jni_str!("getSystemService"),
                    jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
                    &[JValue::Object(&service)],
                )?
                .l()?;
            run(env, &activity, &manager)
        })
    }

    pub(super) fn set(text: &str) -> Result<(), Error> {
        with_manager(|env, _activity, manager| {
            let label = env.new_string("")?;
            let text = env.new_string(text)?;
            let clip = env
                .call_static_method(
                    jni_str!("android/content/ClipData"),
                    jni_str!("newPlainText"),
                    jni_sig!(
                        "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;"
                    ),
                    &[JValue::Object(&label), JValue::Object(&text)],
                )?
                .l()?;
            env.call_method(
                manager,
                jni_str!("setPrimaryClip"),
                jni_sig!("(Landroid/content/ClipData;)V"),
                &[JValue::Object(&clip)],
            )?;
            Ok(())
        })
    }

    pub(super) fn get() -> Result<Option<String>, Error> {
        with_manager(|env, activity, manager| {
            let clip = env
                .call_method(
                    manager,
                    jni_str!("getPrimaryClip"),
                    jni_sig!("()Landroid/content/ClipData;"),
                    &[],
                )?
                .l()?;
            if clip.is_null()
                || env
                    .call_method(&clip, jni_str!("getItemCount"), jni_sig!("()I"), &[])?
                    .i()?
                    == 0
            {
                return Ok(None);
            }
            let item = env
                .call_method(
                    &clip,
                    jni_str!("getItemAt"),
                    jni_sig!("(I)Landroid/content/ClipData$Item;"),
                    &[JValue::Int(0)],
                )?
                .l()?;
            let text = env
                .call_method(
                    &item,
                    jni_str!("coerceToText"),
                    jni_sig!("(Landroid/content/Context;)Ljava/lang/CharSequence;"),
                    &[JValue::Object(activity)],
                )?
                .l()?;
            if text.is_null() {
                return Ok(None);
            }
            let text = env
                .call_method(
                    &text,
                    jni_str!("toString"),
                    jni_sig!("()Ljava/lang/String;"),
                    &[],
                )?
                .l()?;
            let text = env.cast_local::<JString>(text)?;
            Ok(Some(text.to_string()))
        })
    }
}
