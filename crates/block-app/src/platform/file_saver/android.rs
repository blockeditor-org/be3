use std::sync::{Mutex, OnceLock};

use jni::{
    Env, EnvUnowned, Outcome,
    errors::Error as JniError,
    jni_sig, jni_str,
    objects::{JClass, JObject, JString, JValue},
    vm::JavaVM,
};

use super::{Deliver, SaveResult, SavedFile};

static PENDING: OnceLock<Mutex<Option<Deliver<SaveResult>>>> = OnceLock::new();

fn pending() -> &'static Mutex<Option<Deliver<SaveResult>>> {
    PENDING.get_or_init(Default::default)
}

pub(super) fn save(file: SavedFile, deliver: Deliver<SaveResult>) {
    let Ok(mut pending) = pending().lock() else {
        deliver.send(Err("Saving files is unavailable".into()));
        return;
    };
    *pending = Some(deliver);
    if let Err(error) = start(&file)
        && let Some(deliver) = pending.take()
    {
        deliver.send(Err(error));
    }
}

fn start(file: &SavedFile) -> Result<(), String> {
    let context = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
    let started = vm
        .attach_current_thread_for_scope(|env| {
            let activity = unsafe { JObject::from_raw(env, context.context().cast()) };
            let class = main_activity(env, &activity)?;
            let name = env.new_string(&file.name)?;
            let mime_type = env.new_string(&file.mime_type)?;
            let data = env.byte_array_from_slice(&file.data)?;
            env.call_static_method(
                &class,
                jni_str!("saveFile"),
                jni_sig!("(Ljava/lang/String;Ljava/lang/String;[B)Z"),
                &[
                    JValue::Object(&name),
                    JValue::Object(&mime_type),
                    JValue::Object(&data),
                ],
            )?
            .z()
        })
        .map_err(|error: JniError| error.to_string())?;
    if started {
        Ok(())
    } else {
        Err("Saving a file is unavailable right now".into())
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_MainActivity_nativeFileSaved(
    mut env: EnvUnowned<'_>,
    _: JClass<'_>,
    saved: jni::sys::jint,
    error: JString<'_>,
) {
    let result = match env
        .with_env(|_| -> Result<SaveResult, JniError> {
            Ok(match error.is_null() {
                true => Ok(saved != 0),
                false => Err(error.to_string()),
            })
        })
        .into_outcome()
    {
        Outcome::Ok(result) => result,
        Outcome::Err(_) | Outcome::Panic(_) => Err("The file could not be saved".to_owned()),
    };
    let deliver = match pending().lock() {
        Ok(mut pending) => pending.take(),
        Err(_) => return,
    };
    if let Some(deliver) = deliver {
        deliver.send(result);
    }
}

fn main_activity<'local>(
    env: &mut Env<'local>,
    activity: &JObject<'local>,
) -> Result<JClass<'local>, JniError> {
    let class_loader = env
        .call_method(
            activity,
            jni_str!("getClassLoader"),
            jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )?
        .l()?;
    let class_name = env.new_string("com.be3.block.MainActivity")?;
    let class = env
        .call_method(
            &class_loader,
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&class_name)],
        )?
        .l()?;
    env.cast_local::<JClass<'local>>(class)
}
