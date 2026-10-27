use super::*;

#[test]
fn download_pages_escape_values_and_preserve_download_links() {
    let page = download_page(
        "<build>",
        &[Download {
            label: "Linux (x86_64)",
            name: "block&app.zip".into(),
            size: 1_500_000,
            sha256: "<hash>".into(),
        }],
        "/releases/",
    );
    assert!(page.starts_with("<!doctype html><html lang=\"en\">"));
    assert!(page.contains(">&lt;build&gt;</code>"));
    assert!(page.contains("href=\"/releases/block&amp;app.zip\">block&amp;app.zip</a>"));
    assert!(page.contains("<td>1.5 MB</td>"));
    assert!(page.contains(">&lt;hash&gt;</code>"));
}
