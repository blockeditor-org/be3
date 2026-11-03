# A settings block's access comes only from its parent

A block of type `be_block_graph::SETTINGS_TYPE` under a block has Manage when
its parent does, at most View when its parent is viewable, and at most
KnowExists otherwise; its own grants and authorship never raise that, nor
anything below it (`BlockNode::is_block_settings`, `level_below`). Creating,
moving one under a block, or dropping a block's `SETTINGS` reference needs
Manage on the block. No editor lists it among its block's children, and
settings are opened from a block's menu.

## Why

Who may change a setting is ordinary block access: changing settings is a
Manage right on the block they belong to. Opening them from the block's menu
keeps putting settings on a folder rare and deliberate.
