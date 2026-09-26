use jni::EnvUnowned;
use jni::objects::JClass;
use jni::sys::{jfloat, jint};

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_MainActivity_nativeBack(
    _env: EnvUnowned<'_>,
    _: JClass<'_>,
    phase: jint,
    progress: jfloat,
    edge: jint,
) {
    beui::send_android_back(phase, progress, edge);
}
