# Signed bytes keep their own encodings

What is signed is encoded where it is defined: postcard after a domain (access
log entries), a fixed-width concatenation after a domain (escrow epochs,
authorship), chat's own builder, and the host's framed app id ahead of an app's
bytes. Each puts a distinct domain first and its variable-length parts last or
framed, so none is ambiguous.

## Why

The crypto audit suggested one helper for all four. Each encoding is already
unambiguous and checked by its own tests; moving them onto one helper changes
every signed format for no gain in what a signature proves.

## Rejected

- One signed-bytes builder for every signer: the same guarantees, with every
  saved signature made anew.
