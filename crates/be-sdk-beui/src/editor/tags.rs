use be_block::{
    BlockMetadata, TAGGED, Tag, TagColor, TagContent, Tags, TagsContent,
    workspace_settings::retagged,
};
use beui::reactive::*;
use uuid::Uuid;

use super::Editor;
use crate::BlockParent;
use crate::root_settings::RootSetting;

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
