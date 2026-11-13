# Search: what is left of the index library and text extraction

Depends on: none.

The index crate (`crates/be-lib-search`) and the `search-text` request
(guides/type_modules.md, "The protocol") are done; text (Markdown included)
answers one document.

- **More types answer `search-text`**: checklist, database rows, canvas text
  items, the tracker, chat by segment. Each implements
  `BlockContent::search_text` (`Root::search_text_in` for a `Document`); a type
  that loads only its first part reads only the parts asked for and not in
  `indexed`, as the test type `Sections` does. Done per type: a test that it
  answers its documents. The indexer already puts one document per answered
  key; with these, keep each document's `source` and pass the parts the index
  holds text for as `indexed`, so reindexing a large block reads only its
  changed parts.
