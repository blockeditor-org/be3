# Search: what is left of the index library and text extraction

Depends on: none.

The index crate (`crates/be-lib-search`) and the `search-text` request
(guides/type_modules.md, "The protocol") are done; text (Markdown included)
answers one document; checklist and canvas answer one, a database one per
row (the tracker's issues too), chat one for its recent messages and one per
closed segment.

- **PDF text answers `search-text`**: it implements `BlockContent::search_text`
  (`Root::search_text_in` for a `Document`); a type that loads only its first
  part reads only the parts asked for and not in `indexed`, as the test type
  `Sections` and chat do. Done: a test that it answers its documents.
