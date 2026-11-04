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
    let busy = create_memo(move || matches!(content.get(), HostItem::Notice { busy: true, .. }));
    let glyph = create_memo(move || match content.get() {
        HostItem::Notice { icon, .. } => icon,
        HostItem::Unsupported { .. } => None,
    });
    let has_glyph = create_memo(move || glyph.get().is_some());
    let unsupported = matches!(kind, HostItem::Unsupported { .. });
    view! {
        <Frame padding=Space::Lg>
            <List spacing=Space::Md align=Align::Center>
                <Show condition={unsupported}>
                    <Text style=TextStyle::HEADING align=TextAlign::Center>
                        "Unsupported block type"
                    </Text>
                </Show>
                <Show condition={has_glyph}>
                    <Text size=TextSize::Xl color=TextColor::Muted align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Show>
                <Text align=TextAlign::Center color=TextColor::Muted>{text}</Text>
                <Show condition={unsupported}>
                    <Text style=TextStyle::CAPTION align=TextAlign::Center>{detail}</Text>
                </Show>
                <Show condition={busy}>
                    <Progress width=INLINE_PROGRESS_WIDTH />
                </Show>
            </List>
        </Frame>
    }
}
