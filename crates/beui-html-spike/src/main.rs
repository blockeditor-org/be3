#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::io::Result<()> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "index.html".to_owned());
    std::fs::write(out, beui_html_spike::page())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
