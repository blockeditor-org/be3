use beui_core::clipboard::{Clip, HeldClip, PasteAnswer};
use beui_types::ClipItem;
use beui_types::clipboard::formats;

/// The system clipboard. A copy puts its plain text there as the clip's item,
/// and its be3 formats beside it: each format id among the description's
/// MIME types, and its `ClipItem::stored` bytes, as base64, in the
/// description's extras under that id, so another be3 program reads them. The
/// clip a copy wrote stays held for its other representations while its text
/// is still on the clipboard. Written without an emulator to run it on: only
/// its compiling is checked.
#[derive(Default)]
pub struct Clipboard {
    held: HeldClip,
}

impl Clipboard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn copy(&mut self, clip: Clip) {
        let be3 = clip
            .ready_items()
            .into_iter()
            .filter(|item| formats::is_be3(&item.format))
            .collect::<Vec<_>>();
        let _ = android::set(&clip.plain_text().unwrap_or_default(), &be3);
        self.held.copy(clip);
    }

    pub fn paste(&mut self, wanted: &[String], answer: PasteAnswer) {
        let now = android::get().ok().flatten();
        let text = now.as_ref().and_then(|now| now.text.clone());
        let ours = self.held.wrote(text.as_deref());
        if let Some(clip) = self.held.current(ours) {
            return clip.resolve(wanted, move |item| answer.send(item));
        }
        let Some(now) = now else {
            return answer.send(None);
        };
        answer.send(wanted.iter().find_map(|format| now.item(format)));
    }
}

/// What the system clipboard holds: its text, and the be3 formats another be3
/// program put in its description's extras.
struct Held {
    text: Option<String>,
    be3: Vec<(String, String)>,
}

impl Held {
    fn item(&self, format: &str) -> Option<ClipItem> {
        if format == formats::TEXT {
            return self.text.clone().filter(|text| !text.is_empty()).map(ClipItem::text);
        }
        let (_, encoded) = self.be3.iter().find(|(held, _)| held == format)?;
        let stored = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded).ok()?;
        ClipItem::from_stored(format, &stored)
    }
}

mod android {
    use jni::errors::Error;
    use jni::objects::{JObject, JString, JValue};
    use jni::vm::JavaVM;
    use jni::{Env, jni_sig, jni_str};

    fn with_manager<T>(
        run: impl FnOnce(&mut Env<'_>, &JObject<'_>, &JObject<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let context = ndk_context::android_context();
        // unsafe(ffi)
        let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
        vm.attach_current_thread_for_scope(|env| {
            // unsafe(ffi)
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

    pub(super) fn set(text: &str, be3: &[beui_types::ClipItem]) -> Result<(), Error> {
        with_manager(|env, _activity, manager| {
            let label = env.new_string("")?;
            let string_class = env.find_class(jni_str!("java/lang/String"))?;
            let kinds = env.new_object_array(1 + be3.len() as i32, &string_class, JObject::null())?;
            let plain = env.new_string("text/plain")?;
            env.set_object_array_element(&kinds, 0, &plain)?;
            let extras = env.new_object(
                jni_str!("android/os/PersistableBundle"),
                jni_sig!("()V"),
                &[],
            )?;
            for (index, item) in be3.iter().enumerate() {
                let format = env.new_string(&item.format)?;
                env.set_object_array_element(&kinds, 1 + index as i32, &format)?;
                let encoded = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    item.stored(),
                );
                let encoded = env.new_string(&encoded)?;
                env.call_method(
                    &extras,
                    jni_str!("putString"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
                    &[JValue::Object(&format), JValue::Object(&encoded)],
                )?;
            }
            let description = env.new_object(
                jni_str!("android/content/ClipDescription"),
                jni_sig!("(Ljava/lang/CharSequence;[Ljava/lang/String;)V"),
                &[JValue::Object(&label), JValue::Object(&kinds)],
            )?;
            env.call_method(
                &description,
                jni_str!("setExtras"),
                jni_sig!("(Landroid/os/PersistableBundle;)V"),
                &[JValue::Object(&extras)],
            )?;
            let text = env.new_string(text)?;
            let item = env.new_object(
                jni_str!("android/content/ClipData$Item"),
                jni_sig!("(Ljava/lang/CharSequence;)V"),
                &[JValue::Object(&text)],
            )?;
            let clip = env.new_object(
                jni_str!("android/content/ClipData"),
                jni_sig!("(Landroid/content/ClipDescription;Landroid/content/ClipData$Item;)V"),
                &[JValue::Object(&description), JValue::Object(&item)],
            )?;
            env.call_method(
                manager,
                jni_str!("setPrimaryClip"),
                jni_sig!("(Landroid/content/ClipData;)V"),
                &[JValue::Object(&clip)],
            )?;
            Ok(())
        })
    }

    pub(super) fn get() -> Result<Option<super::Held>, Error> {
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
                return Ok(Some(super::Held {
                    text: None,
                    be3: be3_of(env, &clip)?,
                }));
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
            Ok(Some(super::Held {
                text: Some(text.to_string()),
                be3: be3_of(env, &clip)?,
            }))
        })
    }

    /// The be3 formats in a clip's description's extras, by format id.
    fn be3_of(env: &mut Env<'_>, clip: &JObject<'_>) -> Result<Vec<(String, String)>, Error> {
        let description = env
            .call_method(
                clip,
                jni_str!("getDescription"),
                jni_sig!("()Landroid/content/ClipDescription;"),
                &[],
            )?
            .l()?;
        let extras = env
            .call_method(
                &description,
                jni_str!("getExtras"),
                jni_sig!("()Landroid/os/PersistableBundle;"),
                &[],
            )?
            .l()?;
        if extras.is_null() {
            return Ok(Vec::new());
        }
        let count = env
            .call_method(&description, jni_str!("getMimeTypeCount"), jni_sig!("()I"), &[])?
            .i()?;
        let mut be3 = Vec::new();
        for index in 0..count {
            let kind = env
                .call_method(
                    &description,
                    jni_str!("getMimeType"),
                    jni_sig!("(I)Ljava/lang/String;"),
                    &[JValue::Int(index)],
                )?
                .l()?;
            let kind = env.cast_local::<JString>(kind)?;
            let format = kind.to_string();
            if !beui_types::clipboard::formats::is_be3(&format) {
                continue;
            }
            let encoded = env
                .call_method(
                    &extras,
                    jni_str!("getString"),
                    jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                    &[JValue::Object(&kind)],
                )?
                .l()?;
            if encoded.is_null() {
                continue;
            }
            let encoded = env.cast_local::<JString>(encoded)?;
            be3.push((format, encoded.to_string()));
        }
        Ok(be3)
    }
}
