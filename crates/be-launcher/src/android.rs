use std::ffi::CString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use beui::AndroidApp;
use jni::errors::Error as JniError;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::refs::Reference;
use jni::vm::JavaVM;
use jni::{Env, EnvUnowned, jni_sig, jni_str};

use crate::LauncherApp;
use crate::builds::Fetch;
use crate::phone;
use crate::tasks::{Event, Tasks};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const READ_TIMEOUT: Duration = Duration::from_secs(60);

static TASKS: OnceLock<Tasks> = OnceLock::new();

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    let files = app.internal_data_path().unwrap_or_default();
    let shell = shell(&app);
    let options = beui::RunOptions::new("be3 launcher");
    let code = match beui::run_with(options, LauncherApp::new(files, shell)) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("The launcher stopped: {error}");
            1
        }
    };
    std::process::exit(code);
}

fn shell(app: &AndroidApp) -> String {
    let Ok(name) = CString::new("shell") else {
        return String::new();
    };
    let Some(mut asset) = app.asset_manager().open(&name) else {
        return String::new();
    };
    let mut shell = String::new();
    let _ = asset.read_to_string(&mut shell);
    shell.trim().to_owned()
}

pub(crate) fn remember(tasks: Tasks) {
    let _ = TASKS.set(tasks);
}

pub(crate) fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout_read(READ_TIMEOUT)
        .build()
}

pub(crate) fn get(url: &str) -> Result<Option<Vec<u8>>, String> {
    let response = match agent().get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(error) => return Err(format!("{url}: {error}")),
    };
    let mut body = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|error| format!("{url}: {error}"))?;
    Ok(Some(body))
}

pub(crate) struct Http;

impl Fetch for Http {
    fn fetch(&mut self, url: &str, into: &mut dyn std::io::Write) -> Result<(), String> {
        let response = agent()
            .get(url)
            .call()
            .map_err(|error| format!("{url}: {error}"))?;
        std::io::copy(&mut response.into_reader(), into)
            .map(|_| ())
            .map_err(|error| format!("{url}: {error}"))
    }
}

pub(crate) fn launch(build: &Path, data: &Path) -> Result<(), String> {
    let build = build.to_string_lossy().into_owned();
    let data = data.to_string_lossy().into_owned();
    let launched = with_launcher(|env, class| {
        let build = env.new_string(&build)?;
        let data = env.new_string(&data)?;
        env.call_static_method(
            class,
            jni_str!("launch"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Z"),
            &[JValue::Object(&build), JValue::Object(&data)],
        )?
        .z()
    })?;
    if launched {
        Ok(())
    } else {
        Err("The launcher is not open".to_owned())
    }
}

pub(crate) fn stop() {
    let _ = with_launcher(|env, class| {
        env.call_static_method(class, jni_str!("stop"), jni_sig!("()Z"), &[])?
            .z()
    });
}

pub(crate) fn open_url(url: &str) {
    let _ = with_launcher(|env, class| {
        let url = env.new_string(url)?;
        env.call_static_method(
            class,
            jni_str!("openUrl"),
            jni_sig!("(Ljava/lang/String;)Z"),
            &[JValue::Object(&url)],
        )?
        .z()
    });
}

pub(crate) fn install(apk: &Path) -> Result<(), String> {
    let apk = apk.to_string_lossy().into_owned();
    let error = with_launcher(|env, class| {
        let apk = env.new_string(&apk)?;
        let error = env
            .call_static_method(
                class,
                jni_str!("install"),
                jni_sig!("(Ljava/lang/String;)Ljava/lang/String;"),
                &[JValue::Object(&apk)],
            )?
            .l()?;
        if error.is_null() {
            return Ok(None);
        }
        let error = env.cast_local::<JString<'_>>(error)?;
        Ok(Some(error.to_string()))
    })?;
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn with_launcher<T>(
    call: impl for<'local> FnOnce(&mut Env<'local>, &JClass<'local>) -> Result<T, JniError>,
) -> Result<T, String> {
    let context = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
    vm.attach_current_thread_for_scope(|env| {
        let activity = unsafe { JObject::from_raw(env, context.context().cast()) };
        let class = launcher_activity(env, &activity)?;
        call(env, &class)
    })
    .map_err(|error: JniError| error.to_string())
}

fn launcher_activity<'local>(
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
    let class_name = env.new_string("com.be3.launcher.LauncherActivity")?;
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

pub(crate) fn files(tasks: &Tasks) -> PathBuf {
    tasks.root().to_path_buf()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_launcher_LauncherActivity_nativeInstallFinished(
    _env: EnvUnowned<'_>,
    _: JClass<'_>,
    error: JString<'_>,
) {
    let result = if error.is_null() {
        Ok(())
    } else {
        Err(error.to_string())
    };
    if let Some(tasks) = TASKS.get() {
        tasks.send(Event::Phone(phone::Event::Installed(result)));
    }
}
