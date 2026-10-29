use beui::reactive::*;
use beui::styled::*;
use beui::{Align, Color, Direction, NodeId};
use block_editor_beui::BlockInfo;
use block_editor_beui::be_block::{BlockMetadata, TAG_COLOR, TagColor, workspace_settings::tagged};
use database_types::database_schema::ChoiceColor;
use uuid::Uuid;

use crate::{ChoiceColors, use_choice_colors};

pub fn tag_color(colors: ChoiceColors, color: TagColor) -> Color {
    let choice = match color {
        TagColor::Gray => ChoiceColor::Gray,
        TagColor::Blue => ChoiceColor::Blue,
        TagColor::Teal => ChoiceColor::Teal,
        TagColor::Green => ChoiceColor::Green,
        TagColor::Yellow => ChoiceColor::Yellow,
        TagColor::Orange => ChoiceColor::Orange,
        TagColor::Red => ChoiceColor::Red,
        TagColor::Pink => ChoiceColor::Pink,
        TagColor::Purple => ChoiceColor::Purple,
    };
    colors.get(choice)
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShownTag {
    pub id: Uuid,
    pub name: String,
    pub color: TagColor,
}

impl ShownTag {
    pub fn of(info: &BlockInfo) -> Self {
        Self {
            id: info.id,
            name: info.name().unwrap_or_else(|| "Tag".to_owned()),
            color: info.metadata.get(&TAG_COLOR).unwrap_or_default(),
        }
    }
}

pub fn tags_of(metadata: &BlockMetadata) -> Vec<Uuid> {
    tagged(metadata)
}

#[component]
pub fn TagDot(color: Prop<TagColor>, #[prop(default = TextSize::Sm)] size: TextSize) -> NodeId {
    let colors = use_choice_colors();
    let fill = create_memo(move || tag_color(colors, color.get()));
    let dot = Size::Custom((size.icon_px() * 0.6).round());
    view! {
        <Frame width=dot height=dot radius=Radius::Full fill={ColorRole::custom(fill)} />
    }
}

#[component]
pub fn TagChip(tag: Prop<ShownTag>, #[prop(default = TextSize::Sm)] size: TextSize) -> NodeId {
    let tag = create_memo(move || tag.get());
    let color = create_memo(move || tag.with(|tag| tag.color));
    let name = create_memo(move || tag.with(|tag| tag.name.clone()));
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Xs>
            <TagDot color={color} size={size} />
            <Text size={size} color=TextColor::Muted>{name}</Text>
        </List>
    }
}
