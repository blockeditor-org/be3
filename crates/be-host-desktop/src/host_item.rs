use beui::reactive::*;
use beui::styled::*;
use beui::{Align, NodeId, TextAlign};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub enum HostItem {
    Notice {
        text: String,
        busy: bool,
        icon: Option<beui::icons::Glyph>,
    },
    Unsupported {
        block: Uuid,
        block_type: Uuid,
    },
}

#[component]
pub fn HostItemFace(content: Memo<HostItem>) -> NodeId {
    let kind = content.get_untracked();
    let text = create_memo(move || match content.get() {
        HostItem::Notice { text, .. } => text,
        HostItem::Unsupported { block, .. } => format!("Block: {block}"),
    });
    let detail = create_memo(move || match content.get() {
        HostItem::Unsupported { block_type, .. } => format!("Type: {block_type}"),
        _ => String::new(),
    });
    let glyph = create_memo(move || match content.get() {
        HostItem::Notice { icon, .. } => icon,
        HostItem::Unsupported { .. } => None,
    });
    let unsupported = matches!(kind, HostItem::Unsupported { .. });
    view! {
        <Frame padding=Space::Lg>
            <List spacing=Space::Md align=Align::Center>
                {unsupported.then(|| view! {
                    <Text style=TextStyle::HEADING align=TextAlign::Center>
                        "Unsupported block type"
                    </Text>
                })}
                <Show value={move || glyph.with(Option::is_some)}>
                    <Text size=TextSize::Xl color=TextColor::Muted align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Show>
                <Text align=TextAlign::Center color=TextColor::Muted>{text}</Text>
                {unsupported.then(|| view! {
                    <Text style=TextStyle::CAPTION align=TextAlign::Center>{detail}</Text>
                })}
                <Show value={move || matches!(content.get(), HostItem::Notice { busy: true, .. })}>
                    <Progress width=INLINE_PROGRESS_WIDTH />
                </Show>
            </List>
        </Frame>
    }
}
