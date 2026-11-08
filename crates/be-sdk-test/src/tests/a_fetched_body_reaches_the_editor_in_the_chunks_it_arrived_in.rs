use be_sdk_beui::network::{Request, ResponseHead};
use beui::styled::Root;

use super::*;

struct FetchingRoot;

#[component]
impl BeuiRoot for FetchingRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <Fetched editor={editor} />
            </Root>
        }
    }
}

/// Fetches once and shows the status and each chunk of the body as it came.
#[component]
fn Fetched(editor: Editor) -> NodeId {
    let shown = create_signal("waiting".to_owned());
    let request = Request::get("https://api.example.test/items").bearer("token");
    let asked = editor.host().fetch(request);
    editor.then(
        async move {
            let mut response = match asked.await? {
                Ok(response) => response,
                Err(failure) => return Some(failure.to_string()),
            };
            let mut chunks = Vec::new();
            while let Some(chunk) = response.chunk().await {
                match chunk {
                    Ok(chunk) => chunks.push(String::from_utf8_lossy(&chunk).into_owned()),
                    Err(failure) => return Some(failure.to_string()),
                }
            }
            Some(format!("{} {}", response.status, chunks.join("|")))
        },
        move |text| shown.set(text),
    );
    view! {
        <Text @test_id={"fetched"}>{shown}</Text>
    }
}

#[test]
fn a_fetched_body_reaches_the_editor_in_the_chunks_it_arrived_in() {
    let mut test: BeuiTest<FetchingRoot> =
        BeuiTest::new(OwnedEditor::new(EditorHost::default(), Uuid::new_v4()));
    test.run();

    let fetches = test.take_fetches();
    assert_eq!(fetches.len(), 1);
    let request = &fetches[0].request;
    assert_eq!(
        (request.method.as_str(), request.url.as_str()),
        ("GET", "https://api.example.test/items")
    );
    assert_eq!(request.header_value("authorization"), Some("Bearer token"));

    let head = ResponseHead {
        status: 200,
        headers: Vec::new(),
    };
    let chunks = ["one", "two", "three"].map(|chunk| chunk.as_bytes().to_vec());
    test.respond(&fetches[0], head, chunks);
    test.settle_until("the whole body", |test| test.label("fetched") != "waiting");

    assert_eq!(test.label("fetched"), "200 one|two|three");
}
