use super::*;

#[test]
fn download_pages_discourage_crawlers() {
    let html = download_page(
        "build",
        &[Download {
            label: "Linux",
            name: "app.zip".into(),
            size: 100,
            sha256: "hash".into(),
        }],
        "/data/build/app/",
    );
    assert!(html.contains("<meta name=\"robots\" content=\"noindex, nofollow\">"));
    assert!(html.contains("href=\"/data/build/app/app.zip\""));
    assert!(html.contains("href=\"https://be3.pfg.pw/\""));
}
