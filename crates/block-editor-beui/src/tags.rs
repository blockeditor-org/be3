use be_block::{
    BlockMetadata, TAG_COLOR, TAGGED, Tag, TagColor, TagContent, Tags, TagsContent,
    workspace_settings::{retagged, tagged},
};
use beui::reactive::*;
use beui::styled::theme::ThemeKind;
use beui::styled::*;
use beui::{Color32, NodeId};
use block_editor_plugin::root_settings::RootSetting;
use database_types::database_schema::ChoiceColor;
use uuid::Uuid;

use crate::database::choice_color;
use crate::{BlockInfo, BlockParent, Editor};

pub fn tag_color(color: TagColor, kind: ThemeKind) -> Color32 {
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
    choice_color(choice, kind)
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
    let theme = use_theme();
    let fill = create_memo(move || tag_color(color.get(), theme.kind.get()));
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

impl Editor {
    pub fn available_tags(&self, block: Uuid) -> Memo<Vec<Uuid>> {
        let resolved = self.resolved_settings::<Tags>(block);
        create_memo(move || resolved.with(|resolved| resolved.value().tags()))
    }

    pub fn set_tagged(&self, block: Uuid, tag: Uuid, on: bool) {
        let Some(info) = untrack(|| self.blocks().info(block)) else {
            return;
        };
        let tags = retagged(&info.metadata, tag, on);
        let value = (!tags.is_empty()).then_some(tags);
        self.blocks()
            .define(block, TAGGED.definition(value.as_ref()));
    }

    pub fn workspace_tags(&self, block: Uuid) -> Option<Uuid> {
        let owner = untrack(|| self.workspace_of(block))?;
        self.ensure_block_kind::<Tags>(owner, TagsContent::default())
    }

    pub fn personal_tags(&self) -> Option<Uuid> {
        let client = self.host().client_id();
        RootSetting::<TagsContent>::default().ensure(self, client)
    }

    pub fn add_tag(&self, tags: Uuid, name: &str, color: TagColor) -> Uuid {
        let tag = Tag {
            name: name.trim().to_owned(),
            color,
        };
        let content = TagContent::new(&tag);
        let created = self.blocks().create_with(
            <TagContent as be_block::BlockContent>::CONTENT_TYPE,
            Some(be_block::BlockContent::encode(&content)),
            BlockParent::Block(tags),
            BlockMetadata::default(),
        );
        self.content_of::<TagsContent>(tags)
            .operate(Tags::add(created));
        created
    }

    pub fn remove_tag(&self, tags: Uuid, tag: Uuid) {
        self.content_of::<TagsContent>(tags)
            .operate(Tags::remove(tag));
        self.host().delete_block(
            tag,
            <TagContent as be_block::BlockContent>::CONTENT_TYPE,
            BlockParent::Block(tags),
        );
    }
}
