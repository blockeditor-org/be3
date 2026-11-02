use crate::{UnsafeSite, unsafe_sites};

#[test]
fn unsafe_sites_resolves_the_nearest_tag() {
    let source = r#"// unsafe(ffi)
unsafe extern "C" {
    unsafe fn abs(value: i32) -> i32;
}
// unsafe(ffi)
#[unsafe(no_mangle)]
pub extern "C" fn exported(callback: unsafe extern "C" fn()) {}
// unsafe(ffi)
mod boundary {
    fn call() {
        unsafe { abs(1) };
        // unsafe(env)
        unsafe { std::env::set_var("A", "1") };
        let value = match 1 {
            // unsafe(memory)
            1 => unsafe { std::mem::transmute::<u32, f32>(1) },
            _ => 0.0,
        };
    }
}
struct Raw;
// unsafe(send-sync)
unsafe impl Send for Raw {}
unsafe trait Marker {}
"#;

    let site = |category: Option<&str>, line| UnsafeSite {
        category: category.map(str::to_owned),
        line,
    };
    assert_eq!(
        unsafe_sites(source),
        vec![
            site(Some("ffi"), 2),
            site(Some("ffi"), 6),
            site(Some("ffi"), 11),
            site(Some("env"), 13),
            site(Some("memory"), 16),
            site(Some("send-sync"), 23),
            site(None, 24),
        ]
    );
}
