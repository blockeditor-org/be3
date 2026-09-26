use jni::EnvUnowned;
use jni::objects::JClass;
use jni::sys::jint;

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_be3_block_MainActivity_nativeSafeAreaChanged(
    _env: EnvUnowned<'_>,
    _: JClass<'_>,
    left: jint,
    top: jint,
    right: jint,
    bottom: jint,
) {
    beui::set_safe_area(beui::SafeArea {
        left: left as f32,
        top: top as f32,
        right: right as f32,
        bottom: bottom as f32,
    });
}
