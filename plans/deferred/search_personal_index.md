# Search: the personal index

Depends on: plans/search.md.

Set aside from `plans/search.md`: about as large as the rest of search. Until
it lands, blocks shared without their root (Shared with me) and what a
viewer's root index lacks are not content-searchable.

- **One personal index per account** for blocks it can view without viewing
  their root: the Shared with me entry points (a viewable block whose parent is
  not viewable) and their subtrees. A system block of the account beside its
  `Settings` (decide which root it hangs from and its access rule: only the
  account views it), partitioned by entry point: each partition has its own
  segments and document table.
- An entry point inside another of the account's entry points indexes into the
  outer one's partition.
- **Viewers of a root** index what the root's index lacks (stale or missing
  documents found while catching up) into a partition for that root; an entry
  the root's index has a newer commit for is masked at query time and dropped
  by compaction.
- A partition is dropped when its entry point leaves the `Shared` listing, when
  the account gains view of its root (the root's index takes over), or, for a
  viewed root, when the account loses view of it or gains Edit.
- Charged to the account, as its own tree. An account with no personal space to
  save to searches only what the indexes it can read already hold.
- A query fans out over the personal index too.

Alternative set aside: a shared index under each entry point for all its
grantees. Members of the root would index those blocks twice, and the index
would be paid by the tree's owner for people the owner may not know about.
