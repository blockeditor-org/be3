# Search: the index library and text extraction

Depends on: plans/large_blocks.md step 2 (`read-part` under stateless
requests).

The piece `plans/search.md` builds the index block from besides the index
crate (`crates/be-lib-search`, done): a type-module request that answers a
block's searchable text. Left from the crate: fuzzy matching on names, which
needs a way to find a misspelt name's terms without reading a segment's whole
dictionary (the blocked dictionary answers exact terms and prefixes only).

## `search-text`: a type-module request

A stateless request beside `describe` and `to-file` (guides/type_modules.md,
"The protocol"; WIT in `be-sdk-type-api`), backed by a `BlockContent` method
with an empty default, so a type that does not answer is not
content-searchable.

```wit
record search-document {
    key: string,
    /// The part the document's text came from, if it is one part.
    source: option<part-ref>,
    name: option<string>,
    /// None when `source` is in the request's `indexed`.
    body: option<string>,
}

search-text: async func(
    content-type: content-type-id,
    content: loaded,
    keys: option<list<string>>,
    indexed: list<part-ref>,
) -> result<list<search-document>, failure>;
```

`content` is the commit as `loaded` (the first part's bytes and the part
list); a type that loads only its first part reads the others with
`read-part` (served under stateless requests by large_blocks step 2). `keys`
asks for those documents only, so a snippet reads one part of a large block;
none answers every document. `indexed` names parts the indexer already holds
text for: documents from them come back without a body and without reading the
part, so reindexing a large block reads only its changed parts.

**Done**: text and markdown answer one document; a test type with several
parts answers one document per part, asked for one key reads only that part, and with every part but one
in `indexed` reads only that one. Later types: checklist, database rows, canvas text items, the tracker,
chat by segment, PDF text.
