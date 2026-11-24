# Export through artifacts

Depends on: none.

Exporting is written three ways, and none of them is the other's:

- The canvas's `Exporter` (`be-app-infinite-canvas/src/root/export.rs`) writes PNG or SVG to the
  clipboard or a file (`ExportTo::{Clipboard, File, Render}`).
- file-tree's "Export as" (`root/export.rs`) writes a block's own file form or a block file.
- Dynamic artifacts (`editor.blocks().create_artifact`, guides/apps/editor.md "Artifacts") make
  a block from a source block that the app regenerates when the source changes. Only
  logic-grid, a separate project, uses them, so the product does not exercise the mechanism.

The artifact mechanism already has what every export needs: settings (`ArtifactSettings`,
edited inside a `DialogBody`), a way to produce the output from those settings and the source
(regeneration), and an instance in the app's runtime that the host asks to do it. Making it the
only export means exporting into the workspace is as easy as exporting out of be-host, and the
artifact path has real users.

An artifact instance is an `InstanceRole::Artifact` editor instance in the app's one runtime,
beside its other instances; nothing here runs a second copy of an app's wasm.

## Steps

1. **Editors declare exporters.** An exporter has a name, the block type it makes (an image
   for PNG; the workspace's plain file, `AnyFile`, for SVG and other bytes; compiled logic for
   logic-grid), an optional settings component, and a function from settings and the source to
   that type's content. It is the artifact's regeneration, named and listed. Registering one
   gives the editor an `editor.export.<name>` action.
2. **One Export dialog chooses where it goes.**
   - **File:** produce the content once and save the made type's own file form (an image
     block's PNG) out of be-host.
   - **Clipboard:** the same file form, copied.
   - **Workspace:** `create_artifact` with the same settings, regenerated when the source
     changes.
   Settings are edited in the same `ArtifactSettings` component for all three.
3. **The canvas exports through it.** Its PNG and SVG exports become two exporters; its
   `Exporter`, `ExportFormat` and `ExportTo` go (the paste-of-a-copy answers that `Render`
   serves become a call on the PNG exporter).
4. **file-tree's "Export as" lists exporters.** It shows the block type's own file form, the
   block file, and the exporters its editor declares, which it asks the host to run in that
   app's runtime (starting it if it is not running), so exporting works without the editor
   open.
5. **logic-grid's compile is an exporter** to the workspace, as the canvas's are.
6. **The guide follows.** guides/apps/editor.md's "Artifacts" becomes "Exporting", with the
   canvas as its example.

## The test

- Exporting a canvas as PNG to the workspace makes an image block, which changes when the
  canvas does.
- Exporting the same canvas as PNG to a file writes the same bytes as the image block's file
  form.
- file-tree's "Export as" on a canvas offers PNG and SVG with the canvas's editor closed.
