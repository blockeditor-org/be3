# Search: the index library and text extraction

Depends on: plans/large_blocks.md step 2 (`read-part` under stateless
requests), for `search-text` only; `be-lib-search` depends on nothing.

The pieces `plans/search.md` builds the index block from: a pure index crate
and a type-module request that answers a block's searchable text.

## `be-lib-search`

A pure crate, native and wasm, no IO. Everything it reads or writes is bytes
handed in and out, so the index block's type and its tests drive it alike.

- **Tokenizing**: word boundaries from `be-lib-graphemes` (UAX #29 words, once
  it has them; its current word rule until then) and `caseless` folding.
  Prefix matching on the last term; fuzzy matching only on names.
- **Documents** are `(block, key)`: the key is a stable string the block's
  type gives in `search-text` (a chat segment's number, a long text's section
  id, empty for a block that is one document), never a part hash, which
  changes with every edit and every rekey. A newer entry for the same
  `(block, key)` masks the older one. Fields: name, body.
- **Segments**: an immutable batch of documents, saved as separate parts so a
  query reads only what it needs, each a few range reads at most:
  - a term dictionary in sorted blocks of a few KiB, with a small top level of
    each block's first term (an `fst` over those, `fst` being a new
    dependency). A lookup reads the top level and one block;
  - document postings (ids and frequencies) in blocks of 128 with skip data
    and each block's highest score (block-max WAND);
  - positions apart from postings, read only for phrase queries.

  The index stores no text.
- **The document table**: per document, the head commit it was indexed at, the
  part its text came from (`source`, below) and the segment holding it,
  sorted by document; each segment also keeps a
  deletion bitmap, so masking never reads the table.
- **Prefixes**: from two characters, expanding to a bounded number of terms
  (the most frequent).
- **Ranking**: BM25 over body, a strong boost for name hits, recency as a
  tie-break.
- **Compaction** is tiered: segments of similar size merge, so the count stays
  logarithmic and a document is rewritten a logarithmic number of times.
- **Merging two indexes**: a union of segments by id, the newest commit per
  document, the lower watermark. Never conflicts.

Segments, not one index rewritten in place: a batch adds parts without
touching the others, and two devices' batches merge as a union. Rejected for
now: tantivy, which wants a filesystem directory, threads and its own merge
policy, and whose segments the commit DAG cannot three-way merge.

**Done**: unit tests that index a few hundred generated documents in several
batches and find each by a word, a prefix and a phrase; that a reindexed
document's old postings no longer match; that a query counts the dictionary
and postings blocks it read and stays within a small bound while skipping a
common term's list; that compaction and a merge of two indexes give the same
answers as one index built from scratch.

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
