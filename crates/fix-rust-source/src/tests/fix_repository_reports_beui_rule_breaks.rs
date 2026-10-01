use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_reports_beui_rule_breaks() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("lib.rs"),
        r#"fn helper() -> NodeId {
    view! {
        <Frame />
    }
}

#[component]
fn Twice(shown: bool) -> NodeId {
    if shown {
        return view! {
            <Frame />
        };
    }
    view! {
        <Text string="hidden" />
    }
}

#[component]
fn Trailing() -> NodeId {
    let node = view! {
        <Frame />
    };
    node
}

#[component]
fn Rows(keys: Vec<u32>) -> NodeId {
    let total = create_memo(move || {
        count.set(1);
        keys.len()
    });
    view! {
        <List spacing=0.0>
            <ForEach keys>{move |key: u32| view! { <Row key /> }}</ForEach>
        </List>
    }
}

#[component]
fn Nested() -> NodeId {
    view! {
        <Abc def={view! { <Frame /> }} />
    }
}

fn lookup(document: &Document) -> NodeId {
    document.root().unwrap()
}
"#,
    )
    .unwrap();

    let error = fix(&root, true).unwrap_err().to_string();

    assert!(error.contains("component attribute: crates/widget/src/lib.rs:1 `helper`"));
    assert!(
        error.contains("component view: crates/widget/src/lib.rs:7 `Twice` builds more than one")
    );
    assert!(error.contains("component view: crates/widget/src/lib.rs:19 `Trailing` must end"));
    assert!(error.contains("memo write: crates/widget/src/lib.rs:30"));
    assert!(
        !error.contains("`Rows`"),
        "a view inside a row builder is the row's own"
    );
    assert!(
        !error.contains("`Nested`"),
        "a view inside the view is part of it"
    );
    assert!(
        !error.contains("`lookup`"),
        "a function that only finds a node builds nothing"
    );

    fs::remove_dir_all(root).unwrap();
}
