use beui::NodeId;
use beui::reactive::{Direction, ForEach, List, Memo, clone, component, create_memo, view};
use beui::styled::{Body, Caption};
use block_plugin_api::VersionCommit;

pub fn short_id(id: &[u8; 32]) -> String {
    id.iter()
        .take(4)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[component]
pub fn VersionHistory(log: Memo<Vec<VersionCommit>>) -> NodeId {
    let rows = create_memo(clone!(log -> move || {
        log.with(|log| {
            log.iter()
                .map(|commit| (commit.id, commit.message.clone()))
                .collect::<Vec<_>>()
        })
    }));
    let empty = create_memo(clone!(rows -> move || rows.with(Vec::is_empty)));
    view! {
        <List spacing=6.0>
            <beui::reactive::Show condition={empty}>
                <Caption content="Nothing has been committed yet." />
            </beui::reactive::Show>
            <ForEach keys={rows}>
                {move |(id, message): ([u8; 32], String)| {
                    let short = short_id(&id);
                    let message = match message.is_empty() {
                        true => "(no message)".to_owned(),
                        false => message,
                    };
                    view! {
                        <List
                            direction=Direction::Horizontal
                            spacing=8.0
                            @test_id={format!("version.commit.{short}")}
                        >
                            <Caption content={short} />
                            <Body content={message} />
                        </List>
                    }
                }}
            </ForEach>
        </List>
    }
}
