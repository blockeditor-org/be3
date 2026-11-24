# One way each across the apps

Depends on: none. The stage and side panel layout copied into many apps is
plans/dock_defaults.md; export is plans/export_through_artifacts.md.

Pieces several editors write for themselves, which be-sdk-beui or the types should hold once.

## Steps

1. **File types say their files; one replace panel.** Image (`ImageEditor`), audio
   (`AudioPanel`), pdf (`DetailsPanel`) and the game module (`ModulePanel`) each build a
   "Replace …" panel (`FileChooser::new(filter(), imported)`, `on_chosen` → `replace_content`,
   disabled on `busy || read_only`, a danger caption) and repeat `filter()` and `imported()` in
   their `root.rs`. Audio has drifted: its `root.rs` keeps its own extensions and
   `guess_media_type` (m4a as audio/mp4), while `AudioFile`'s `BlobFile` uses
   `be_block_content::plain::media_type_of`, which knows no m4a or oga, so a picked .m4a is
   accepted but "Import a file…" does not claim one. Every `BlobFile` type states
   `FILE_EXTENSIONS` and `MIME_TYPES` (image and the game module already do), the media table
   takes them, and a generic `from_file` goes through `BlobFile::header`. be-sdk-beui gets
   `ContentFileReplace` beside `ContentFileCreation` (`file_chooser.rs`), and the four panels
   use it.
2. **One block-tree model.** file-tree (`root/rows.rs`: `Tree`, `Row`, `push_section`,
   `push_shared`, `push_inside`), the workspace picker (`picker/model.rs`: `Walk`, `TreeRow`,
   `link_rows`, `place_rows`) and workspace-index (`root/entries.rs`: `Entry`, `Listing`) each
   walk sections then nested blocks, with their own unfold state, labels and `type_name`; only
   which sections exist is shared (`be_sdk_beui::sections`). One model in be-sdk-beui: rows
   keyed by path, an unfold policy and filters, fed either by live per-folder watches
   (file-tree) or a `Graph` snapshot (the picker). The three render it, and workspace-index
   gets file-tree's row menu (rename, delete, move, export) instead of only accepting drops.
3. **Typed workers.** be-worker-map and be-worker-pdf hand-write postcard encoding both ways
   over the bytes-only `Work` and `Shared` (be-sdk-worker), and the map's `tiles.rs`
   (`TileWorker`: `sent`, `by_generation`, `rasterizing`) and the pdf's `render.rs`
   (`RenderClient.sent`) each work out which jobs a stopped generation will never answer.
   be-sdk-worker gets `Shared<Job, Answer>` and `Work<In, Out>`, which do the serde, give each
   job a ticket and fail the tickets a stopped generation leaves; both apps use them.
4. **A block name's length is checked once.** be-type-text defines its own `MAX_NAME_BYTES`
   beside `be_block_content::metadata::MAX_NAME_BYTES`, file-tree's rename (`clipped()` in
   `root/rename.rs`) cuts a long name silently, and the workspace's rename dialog refuses it.
   `Blocks::set_name` refuses a name past the limit and both renames show that refusal; the
   text type uses the shared constant.

## The test

- "Import a file…" with an .m4a file makes an audio block.
- workspace-index's row menu renames a block, as file-tree's does.
