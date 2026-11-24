# One way each across the apps

Depends on: none. The stage and side panel layout copied into many apps is
plans/dock_defaults.md; export is plans/export_through_artifacts.md.

Pieces several editors write for themselves, which be-sdk-beui or the types should hold once.

## Steps

1. **One block-tree model.** file-tree (`root/rows.rs`: `Tree`, `Row`, `push_section`,
   `push_shared`, `push_inside`), the workspace picker (`picker/model.rs`: `Walk`, `TreeRow`,
   `link_rows`, `place_rows`) and workspace-index (`root/entries.rs`: `Entry`, `Listing`) each
   walk sections then nested blocks, with their own unfold state, labels and `type_name`; only
   which sections exist is shared (`be_sdk_beui::sections`). One model in be-sdk-beui: rows
   keyed by path, an unfold policy and filters, fed either by live per-folder watches
   (file-tree) or a `Graph` snapshot (the picker). The three render it, and workspace-index
   gets file-tree's row menu (rename, delete, move, export) instead of only accepting drops.

## The test

- workspace-index's row menu renames a block, as file-tree's does.
