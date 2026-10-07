#[cfg(unix)]
pub(crate) fn utc_offset() -> i32 {
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    let mut local = unsafe { std::mem::zeroed::<libc::tm>() };
    match unsafe { libc::localtime_r(&now, &mut local) }.is_null() {
        true => 0,
        false => i32::try_from(local.tm_gmtoff).unwrap_or(0),
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn utc_offset() -> i32 {
    let minutes_behind = js_sys::Date::new_0().get_timezone_offset();
    (-minutes_behind * 60.0) as i32
}

#[cfg(not(any(unix, target_arch = "wasm32")))]
pub(crate) fn utc_offset() -> i32 {
    0
}
