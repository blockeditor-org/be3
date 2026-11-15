use beui_core::clipboard::{Clip, HeldClip, PasteAnswer};
use beui_types::ClipItem;
use beui_types::clipboard::formats;

/// How many UTF-16 code units a clip may take on the system clipboard, its
/// text and the be3 formats' base64 in its extras together: a Parcel holds
/// strings as UTF-16, and a clip crosses Binder, whose transactions stop near
/// 1 MB, past which `setPrimaryClip` throws. The plain text has first claim:
/// beyond it, it goes there cut short with `CUT_SHORT` after it, and the be3
/// formats take what it leaves, those beyond it staying in the held clip, for
/// pastes in this program.
const PARCEL_UNITS: usize = 400 * 1024;

/// What ends text cut short to fit the clipboard.
const CUT_SHORT: &str = "\n\n[Cut short: this copy was too large for the clipboard. \
Paste it in the program it was copied from for all of it.]";

/// The extras key of the copy's nonce, which tells this program's clip from
/// another's.
const NONCE: &str = "be3/copy";

/// The system clipboard. A copy puts its plain text there as the clip's item,
/// and its be3 formats beside it: each format id among the description's
/// MIME types, and its `ClipItem::stored` bytes, as base64, in the
/// description's extras under that id, so another be3 program reads them,
/// with a nonce of this copy. The clip a copy wrote stays held for its other
/// representations while the clipboard's nonce is still its own, or, when
/// the copy could not be put there at all, until the clipboard changes. Written
/// without an emulator to run it on: only its compiling is checked.
#[derive(Default)]
pub struct Clipboard {
    held: HeldClip,
    mark: Option<Mark>,
}

/// How a paste tells the clipboard is still the held clip's.
enum Mark {
    /// The copy's nonce is in the clipboard's extras.
    Nonce(i64),
    /// The copy could not be put there, so the held clip answers pastes in
    /// this program until the clipboard changes from what it then held.
    Unchanged(Option<(Option<String>, Option<i64>)>),
}

impl Mark {
    fn of(held: Option<&Held>) -> Option<(Option<String>, Option<i64>)> {
        held.map(|held| (held.text.clone(), held.nonce))
    }

    fn ours(&self, now: Option<&Held>) -> bool {
        match self {
            Mark::Nonce(nonce) => now.is_some_and(|now| now.nonce == Some(*nonce)),
            Mark::Unchanged(then) => *then == Mark::of(now),
        }
    }
}

impl Clipboard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn copy(&mut self, clip: Clip) {
        let encoded = clip
            .ready_items()
            .into_iter()
            .filter(|item| formats::is_be3(&item.format))
            .map(|item| {
                let stored = item.stored();
                let encoded =
                    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, stored);
                (item.format, encoded)
            });
        let text = clip.plain_text().unwrap_or_default();
        let text = beui_types::clip::cut_short(&text, PARCEL_UNITS, CUT_SHORT);
        let left = PARCEL_UNITS - beui_types::clip::utf16_units(&text);
        let fitting = beui_types::clip::fitting(encoded, left, |(_, encoded)| encoded.len());
        let nonce = fresh_nonce();
        // The nonce goes in either way, so a paste here still knows the
        // clipboard is this copy's when its text there was cut short.
        let set = android::set(&text, &fitting, nonce).or_else(|_| android::set(&text, &[], nonce));
        self.mark = Some(match set {
            Ok(()) => Mark::Nonce(nonce),
            Err(_) => Mark::Unchanged(Mark::of(android::get().ok().flatten().as_ref())),
        });
        self.held.copy(clip);
    }

    pub fn paste(&mut self, wanted: &[String], answer: PasteAnswer) {
        let now = android::get().ok().flatten();
        let ours = self
            .mark
            .as_ref()
            .is_some_and(|mark| mark.ours(now.as_ref()));
        if let Some(clip) = self.held.current(ours) {
            return clip.resolve(wanted, move |item| answer.send(item));
        }
        let Some(now) = now else {
            return answer.send(None);
        };
        answer.send(wanted.iter().find_map(|format| now.item(format)));
    }
}

/// A nonce no other copy is likely to have drawn.
fn fresh_nonce() -> i64 {
    uuid::Uuid::new_v4().as_u64_pair().0 as i64
}

/// What the system clipboard holds: its text, the be3 formats a be3 program
/// put in its description's extras, and that copy's nonce.
struct Held {
    text: Option<String>,
    be3: Vec<(String, String)>,
    nonce: Option<i64>,
}

impl Held {
    fn item(&self, format: &str) -> Option<ClipItem> {
        if format == formats::TEXT {
            return self
                .text
                .clone()
                .filter(|text| !text.is_empty())
                .map(ClipItem::text);
        }
        let (_, encoded) = self.be3.iter().find(|(held, _)| held == format)?;
        let stored =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded).ok()?;
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
            // A frame of its own, so the references made here go with it on
            // the main thread, which stays attached.
            let ran = env.with_local_frame(32, |env| {
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
            });
            // A Java exception left pending would abort the next JNI call.
            if ran.is_err() && env.exception_check() {
                env.exception_clear();
            }
            ran
        })
    }

    /// Puts `text` on the clipboard, and the copy's `nonce` and the be3
    /// formats as (format id, base64 of its stored bytes) in the description's
    /// extras.
    pub(super) fn set(text: &str, be3: &[(String, String)], nonce: i64) -> Result<(), Error> {
        with_manager(|env, _activity, manager| {
            let label = env.new_string("")?;
            let string_class = env.find_class(jni_str!("java/lang/String"))?;
            let kinds =
                env.new_object_array(1 + be3.len() as i32, &string_class, JObject::null())?;
            let plain = env.new_string("text/plain")?;
            kinds.set_element(env, 0, &plain)?;
            let extras = env.new_object(
                jni_str!("android/os/PersistableBundle"),
                jni_sig!("()V"),
                &[],
            )?;
            for (index, (format, encoded)) in be3.iter().enumerate() {
                let format = env.new_string(format)?;
                kinds.set_element(env, 1 + index, &format)?;
                let encoded = env.new_string(encoded)?;
                env.call_method(
                    &extras,
                    jni_str!("putString"),
                    jni_sig!("(Ljava/lang/String;Ljava/lang/String;)V"),
                    &[JValue::Object(&format), JValue::Object(&encoded)],
                )?;
            }
            let key = env.new_string(super::NONCE)?;
            env.call_method(
                &extras,
                jni_str!("putLong"),
                jni_sig!("(Ljava/lang/String;J)V"),
                &[JValue::Object(&key), JValue::Long(nonce)],
            )?;
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
                let (be3, nonce) = extras_of(env, &clip)?;
                return Ok(Some(super::Held {
                    text: None,
                    be3,
                    nonce,
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
            let (be3, nonce) = extras_of(env, &clip)?;
            Ok(Some(super::Held {
                text: Some(beui_types::clip::text_from_system(text.to_string())),
                be3,
                nonce,
            }))
        })
    }

    /// The be3 formats in a clip's description's extras, by format id, and
    /// the nonce of the copy that put them there.
    #[allow(clippy::type_complexity)]
    fn extras_of(
        env: &mut Env<'_>,
        clip: &JObject<'_>,
    ) -> Result<(Vec<(String, String)>, Option<i64>), Error> {
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
            return Ok((Vec::new(), None));
        }
        let key = env.new_string(super::NONCE)?;
        let has_nonce = env
            .call_method(
                &extras,
                jni_str!("containsKey"),
                jni_sig!("(Ljava/lang/String;)Z"),
                &[JValue::Object(&key)],
            )?
            .z()?;
        let nonce = match has_nonce {
            true => Some(
                env.call_method(
                    &extras,
                    jni_str!("getLong"),
                    jni_sig!("(Ljava/lang/String;)J"),
                    &[JValue::Object(&key)],
                )?
                .j()?,
            ),
            false => None,
        };
        let count = env
            .call_method(
                &description,
                jni_str!("getMimeTypeCount"),
                jni_sig!("()I"),
                &[],
            )?
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
        Ok((be3, nonce))
    }
}
