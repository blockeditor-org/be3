use be_sdk_beui::worker::Hosted;
use be_worker_pdf::{Answer, Document, Job, Measure, Target, Unrendered};
use be_worker_pdf_render::Renderer;

use super::*;

fn ask(renderer: &mut Hosted<Renderer>, document: Document, data: Option<Vec<u8>>) -> Answer {
    let mut answers = renderer
        .ask(&Job {
            document,
            page: 0,
            target: Target::FullPage {
                max_width: 100.0,
                max_height: 100.0,
            },
            data,
        })
        .expect("the renderer takes the job");
    assert_eq!(answers.len(), 1, "the renderer answers each job once");
    answers.remove(0)
}

fn loaded(answer: &Answer) -> bool {
    answer
        .measures
        .iter()
        .any(|measure| matches!(measure, Measure::Duration(name, _) if name == "Document load"))
}

/// The renderer keeps the document a job opened, so a later job of the same document carries no
/// bytes and loads nothing, and one of a document it never opened is answered `Unopened`.
#[test]
fn the_renderer_renders_twice_from_one_open() {
    let mut renderer = Hosted::<Renderer>::start(&mut Vec::new()).expect("the renderer starts");
    let document = Document {
        client: 1,
        revision: 7,
    };

    let first = ask(&mut renderer, document, Some(document_bytes()));
    assert!(first.result.is_ok(), "the first job renders");
    assert!(loaded(&first), "the first job opens the document");

    let second = ask(&mut renderer, document, None);
    assert!(
        second.result.is_ok(),
        "the second job renders from the open document"
    );
    assert!(!loaded(&second), "the second job loads nothing");

    let other = Document {
        client: 1,
        revision: 8,
    };
    assert!(matches!(
        ask(&mut renderer, other, None).result,
        Err(Unrendered::Unopened)
    ));
}

fn document_bytes() -> Vec<u8> {
    document("0 0 0 rg 20 20 160 50 re f")
}
