//! Push notifications on Android, through a UnifiedPush distributor
//! (decisions/android_push_via_unifiedpush.md, plans/notifications.md). The Java side
//! (`android/java/com/be3/host`) registers with the distributor and receives its broadcasts in
//! `PushReceiver`, which may run with no activity: it loads this library and calls the
//! `native*` entries here, which work from the files directory alone. The endpoint the
//! distributor gives is kept in the host state with this device's RFC 8291 keys
//! (`be_host_stack::device_push`) and registered with the server for each organization the host
//! opens (`sync`).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use be_host_base::host_state::HostStateStore;
use be_host_base::push_device::{PushDevice, set_state};
use jni::errors::{Error as JniError, LogErrorAndDefault};
use jni::objects::{JByteArray, JClass, JObject, JString, JValue};
use jni::sys::jstring;
use jni::vm::JavaVM;
use jni::{EnvUnowned, jni_sig, jni_str};
use uuid::Uuid;

/// Set from any thread when the distributor's endpoint or the activity's notification state
/// changed: the host reads both again on its next frame (`changed`).
static CHANGED: AtomicBool = AtomicBool::new(false);

fn note_changed() {
    CHANGED.store(true, Ordering::SeqCst);
    be_host_base::host::wake();
}

/// Whether the endpoint or the state changed since the host last looked; reads the state again
/// when it did.
pub(crate) fn changed() -> bool {
    let changed = CHANGED.swap(false, Ordering::SeqCst);
    if changed {
        refresh();
    }
    changed
}

fn state_in(files: &str) -> Option<HostStateStore> {
    HostStateStore::open(Path::new(files).join("host.sqlite3"))
        .map_err(|error| eprintln!("be-host: the host state did not open for a push: {error}"))
        .ok()
}

/// Calls `method` on the activity (`MainActivity`), whose class this thread may not find by
/// name: a thread Rust started has only the system's class loader.
fn on_activity<T>(
    call: impl FnOnce(&mut jni::Env<'_>, &JObject<'_>) -> Result<T, JniError>,
) -> Result<T, JniError> {
    let context = ndk_context::android_context();
    // unsafe(ffi)
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
    vm.attach_current_thread_for_scope(|env| {
        // unsafe(ffi)
        let activity = unsafe { JObject::from_raw(env, context.context().cast()) };
        call(env, &activity)
    })
}

/// Reads whether this device shows pushes from the activity (`MainActivity.pushState`).
fn refresh() {
    let state = on_activity(|env, activity| {
        env.call_method(activity, jni_str!("pushState"), jni_sig!("()I"), &[])?
            .i()
    });
    set_state(match state {
        Ok(1) => PushDevice::Off,
        Ok(2) => PushDevice::On,
        Ok(3) => PushDevice::Denied,
        _ => PushDevice::Unsupported,
    });
}

/// Lets apps turn pushes on this device on and off (`host-services.set-push-device`): on asks
/// the activity to ask for the permission to notify and to register with the distributor,
/// with the server's VAPID key once the server gives it, since some push services take pushes
/// only signed with the key an endpoint was made for.
pub(crate) fn install() {
    be_host_base::push_device::install(std::rc::Rc::new(|on, reply| {
        if on {
            // With no stack running there is no server to ask (and no answer would come).
            if be_host_stack::running_organization().is_some() {
                be_host_stack::push_key(|key| enable(key.ok().map(|key| base64url(&key))));
            } else {
                enable(None);
            }
            reply(Ok(()));
            return;
        }
        let disabled = on_activity(|env, activity| {
            env.call_method(activity, jni_str!("pushDisable"), jni_sig!("()V"), &[])?;
            Ok(())
        });
        refresh();
        reply(disabled.map_err(|error| error.to_string()));
    }));
    refresh();
}

/// Asks the activity to turn pushes on (`MainActivity.pushEnable`), from any thread.
fn enable(vapid: Option<String>) {
    let enabled = on_activity(|env, activity| {
        let vapid = match &vapid {
            Some(vapid) => JObject::from(env.new_string(vapid)?),
            None => JObject::null(),
        };
        env.call_method(
            activity,
            jni_str!("pushEnable"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&vapid)],
        )?;
        Ok(())
    });
    if let Err(error) = enabled {
        eprintln!("be-host: pushes were not turned on: {error}");
    }
    note_changed();
}

fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let joined = chunk
            .iter()
            .enumerate()
            .fold(0u32, |joined, (at, byte)| joined | u32::from(*byte) << (16 - 8 * at));
        for at in 0..=chunk.len() {
            text.push(char::from(ALPHABET[(joined >> (18 - 6 * at)) as usize & 63]));
        }
    }
    text
}

/// Registers the distributor's endpoint with the server for the organization the stack is open
/// in. An endpoint is registered per organization, so this runs for each one the host opens,
/// and again when the distributor gives a new endpoint.
pub(crate) fn sync(state: &HostStateStore) {
    let Ok(Some(address)) = state.push_endpoint() else {
        return;
    };
    let keys = match be_host_stack::device_push::keys(state) {
        Ok(keys) => keys,
        Err(error) => {
            eprintln!("be-host: the push keys are not usable: {error}");
            return;
        }
    };
    let endpoint = be_host_stack::device_push::endpoint(&keys, address);
    be_host_stack::register_push_endpoint(endpoint, |registered| {
        if let Err(error) = registered {
            eprintln!("be-host: the push endpoint was not registered: {error}");
        }
    });
}

/// Signing out of an account on this device: unregisters from the distributor when
/// `unsubscribe` (no saved account is left to use it), then ends the account's session naming
/// the endpoint, which the server forgets for the account in every organization, as
/// `web_push::sign_out` does.
pub(crate) fn sign_out(state: &HostStateStore, server_url: String, token: String, unsubscribe: bool) {
    let address = state.push_endpoint().ok().flatten();
    if unsubscribe {
        let disabled = on_activity(|env, activity| {
            env.call_method(activity, jni_str!("pushDisable"), jni_sig!("()V"), &[])?;
            Ok(())
        });
        if let Err(error) = disabled {
            eprintln!("be-host: the push distributor was not told to unregister: {error}");
        }
        if let Err(error) = state.set_push_endpoint(None) {
            eprintln!("be-host: the push endpoint was not forgotten: {error}");
        }
        refresh();
    }
    be_host_base::platform::spawn_request(async move {
        let _ = be_host_stack::accounts::logout(server_url, token, address).await;
    });
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_host_PushReceiver_nativeNewEndpoint<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    files: JString<'local>,
    endpoint: JString<'local>,
) {
    env.with_env(|env| -> Result<(), JniError> {
        let files = files.try_to_string(env)?;
        let endpoint = endpoint.try_to_string(env)?;
        if let Some(state) = state_in(&files) {
            // The keys are made before the endpoint is kept, which keeps it only beside them.
            let kept = be_host_stack::device_push::keys(&state)
                .map_err(|error| error.to_string())
                .and_then(|_| {
                    state
                        .set_push_endpoint(Some(&endpoint))
                        .map_err(|error| error.to_string())
                });
            if let Err(error) = kept {
                eprintln!("be-host: the push endpoint was not kept: {error}");
            }
        }
        note_changed();
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
}

// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_host_PushReceiver_nativeUnregistered<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    files: JString<'local>,
) {
    env.with_env(|env| -> Result<(), JniError> {
        let files = files.try_to_string(env)?;
        if let Some(state) = state_in(&files)
            && let Err(error) = state.set_push_endpoint(None)
        {
            eprintln!("be-host: the push endpoint was not forgotten: {error}");
        }
        note_changed();
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
}

/// Opens a push the distributor handed over (`be_host_stack::device_push::receive`): the
/// notification's title, body, organization and block joined by NUL (the last two empty for one
/// naming nothing), or null for a push already shown.
// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_host_PushReceiver_nativeMessage<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    files: JString<'local>,
    body: JByteArray<'local>,
) -> jstring {
    env.with_env(|env| -> Result<jstring, JniError> {
        let files = files.try_to_string(env)?;
        let body = env.convert_byte_array(&body)?;
        let shown = match state_in(&files) {
            Some(state) => be_host_stack::device_push::receive(
                &state,
                Path::new(&files),
                &body,
                now_milliseconds(),
            ),
            None => Some(be_host_stack::device_push::Shown::generic()),
        };
        let Some(shown) = shown else {
            return Ok(std::ptr::null_mut());
        };
        let (organization, block) = shown.opens.map_or((String::new(), String::new()), |opens| {
            (opens.0.to_string(), opens.1.to_string())
        });
        let joined = [shown.title, shown.body, organization, block].join("\0");
        Ok(env.new_string(joined)?.into_raw())
    })
    .resolve::<LogErrorAndDefault>()
}

fn now_milliseconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(i64::MAX))
}

/// A notification was pressed (`MainActivity`): the host opens its block.
// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_host_MainActivity_nativeOpen<'local>(
    mut env: EnvUnowned<'local>,
    _: JClass<'local>,
    organization: JString<'local>,
    block: JString<'local>,
) {
    env.with_env(|env| -> Result<(), JniError> {
        let organization = organization.try_to_string(env)?;
        let block = block.try_to_string(env)?;
        if let (Ok(organization), Ok(block)) = (organization.parse::<Uuid>(), block.parse::<Uuid>())
        {
            crate::pressed_notifications::pressed(organization, block);
        }
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
}

/// The permission to notify was answered, or the distributor was asked to register or
/// unregister: the host reads the state again.
// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_host_MainActivity_nativePushChanged<'local>(
    _env: EnvUnowned<'local>,
    _: JClass<'local>,
) {
    note_changed();
}
