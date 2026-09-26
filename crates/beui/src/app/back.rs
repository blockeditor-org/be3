use jni::objects::{JObject, JValue};
use jni::vm::JavaVM;
use jni::{jni_sig, jni_str};

use crate::input::{BackEdge, BackGesture};

const STARTED: i32 = 0;
const PROGRESSED: i32 = 1;
const CANCELLED: i32 = 2;
const INVOKED: i32 = 3;
const EDGE_LEFT: i32 = 0;
const EDGE_RIGHT: i32 = 1;

pub fn send_android_back(phase: i32, progress: f32, edge: i32) {
    let edge = match edge {
        EDGE_LEFT => BackEdge::Left,
        EDGE_RIGHT => BackEdge::Right,
        _ => BackEdge::None,
    };
    let gesture = match phase {
        STARTED => BackGesture::Started { edge },
        PROGRESSED => BackGesture::Progressed(progress),
        CANCELLED => BackGesture::Cancelled,
        INVOKED => BackGesture::Invoked,
        _ => return,
    };
    super::native::send_back(gesture);
}

pub(super) fn set_handled(handled: bool) {
    let context = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) };
    let result = vm.attach_current_thread_for_scope(|env| {
        let activity = unsafe { JObject::from_raw(env, context.context().cast()) };
        env.call_method(
            &activity,
            jni_str!("setBackHandled"),
            jni_sig!("(Z)V"),
            &[JValue::Bool(handled)],
        )
        .map(|_| ())
    });
    if let Err(error) = result {
        eprintln!("Could not hand the back gesture to the activity: {error}");
    }
}
