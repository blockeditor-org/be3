use std::sync::{Mutex, OnceLock};

use jni::{
    Env, EnvUnowned, Outcome,
    errors::Error as JniError,
    jni_sig, jni_str,
    objects::{JByteArray, JClass, JObject, JString, JValue},
    refs::Reference,
    vm::JavaVM,
};

use super::{Deliver, FileFilter, PickResult, PickedFile};

static PENDING: OnceLock<Mutex<Option<Deliver<PickResult>>>> = OnceLock::new();

fn pending() -> &'static Mutex<Option<Deliver<PickResult>>> {
    PENDING.get_or_init(Default::default)
}

pub(super) fn open(filter: &FileFilter, deliver: Deliver<PickResult>) {
    let Ok(mut pending) = pending().lock() else {
        deliver.send(Err("The file picker is unavailable".into()));
        return;
    };
    *pending = Some(deliver);
    if let Err(error) = start(filter)
        && let Some(deliver) = pending.take()
    {
        deliver.send(Err(error));
    }
}

fn start(filter: &FileFilter) -> Result<(), String> {
    let context = ndk_context::android_context();

    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
    let started = vm
        .attach_current_thread_for_scope(|env| {
            let activity = unsafe { JObject::from_raw(env, context.context().cast()) };
            let class = main_activity(env, &activity)?;
            let mime_types = env.new_string(filter.mime_types.join(","))?;
            env.call_static_method(
                &class,
                jni_str!("pickFile"),
                jni_sig!("(Ljava/lang/String;)Z"),
                &[JValue::Object(&mime_types)],
            )?
            .z()
        })
        .map_err(|error: JniError| error.to_string())?;
    if started {
        Ok(())
    } else {
        Err("Choosing a file is unavailable right now".into())
    }
}

pub(crate) fn main_activity<'local>(
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

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_MainActivity_nativeFilePicked(
    mut env: EnvUnowned<'_>,
    _: JClass<'_>,
    name: JString<'_>,
    data: JByteArray<'_>,
    error: JString<'_>,
) {
    let result = match env
        .with_env(|env| collect(env, &name, &data, &error))
        .into_outcome()
    {
        Outcome::Ok(result) => result,
        Outcome::Err(_) | Outcome::Panic(_) => Err("The chosen file could not be read".to_owned()),
    };
    let deliver = match pending().lock() {
        Ok(mut pending) => pending.take(),
        Err(_) => return,
    };
    if let Some(deliver) = deliver {
        deliver.send(result);
    }
}

fn collect(
    env: &mut Env<'_>,
    name: &JString<'_>,
    data: &JByteArray<'_>,
    error: &JString<'_>,
) -> Result<PickResult, JniError> {
    if !error.is_null() {
        return Ok(Err(error.to_string()));
    }
    if data.is_null() {
        return Ok(Ok(None));
    }
    let name = if name.is_null() {
        String::new()
    } else {
        name.to_string()
    };
    let data = env.convert_byte_array(data)?;
    Ok(Ok(Some(PickedFile { name, data })))
}
