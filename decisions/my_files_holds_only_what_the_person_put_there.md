# My files holds only what the person put there

A block made without a place the person chose goes to the account's Unfiled,
a derived root per account per organization (`be_block::unfiled`), never to My
files. Unfiled has no content: it holds every block whose parent names it, so
apps and the host can file a block there without editing anything another
person must be able to edit. Apps name it with `block-location::unfiled` and
list it with `block-query::unfiled`, and never learn its id. Only the
account's own Unfiled is hidden this way: another account's, which an
administrator of a company organization sees as they see that account's My
files, is a block like any other.

## Why

Blocks an app makes on its own (a view, a terminal, an export) are not the
person's filing. Putting them in My files fills the folders they arrange with
things they never placed. One reserved place keeps them findable, and moving a
block out of it is how it gets filed; references are by id, so filing breaks
nothing.

## Rejected

- Leaving them detached: a detached block is in the trash and is reclaimed.
- Listing Unfiled's children in a content like a folder's: every block made
  there would be an edit to one shared block, and liveness would depend on it.
