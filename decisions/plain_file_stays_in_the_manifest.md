# A type's plain-file form stays in its manifest

A block type's manifest says whether it has a plain-file form (`plain_file`),
and its type module says the same in `type-info.plain`. be-tool-cli's
`a_manifest_says_which_of_its_types_have_a_plain_file` holds the two in step.

## Why

- The host starts a declared type module lazily, on the first call to it, but
  apps need the plain-file form before that: the file tree offers "Export as"
  from it and desktop programs match the files they open by it. The manifest
  is what the host knows before any module runs.
- `be` and `be sync` run type modules without manifests and read
  `Plain::Directory` and `Plain::File` from them (`plain_of`), so the module's
  answer stays too.

## Rejected

- Filling the catalog's plain types from the type registry as modules start
  (plans/api_simplification.md once proposed it): until a module ran, its
  types offered no export and opened in no program, and the catalog was
  resent to every app at each module start.
- Starting every declared module after sign-in, which spends what lazy
  starting saves, and asking the module when an app first asks, which turns a
  menu's contents into an asynchronous host call.
