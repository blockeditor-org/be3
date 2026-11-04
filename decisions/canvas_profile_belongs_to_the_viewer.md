# A canvas profile belongs to the viewer, not the document

The infinite canvas has profiles (Prototyping, Vector, Raster, and later Interactivity): each is a way of using the editor, with its own tools, chrome and defaults, over the same document format, so a sketch made in one can be finished in another. The profile in use lives in the editor's `CanvasState`, and switching it never changes a peer's profile. The document stores only the last profile used (`Canvas::profile`), as a viewer default so reopening it comes back the same, and that write is kept out of undo.

## Why

- One document format for every profile means a profile adds tools, not a second model.
- The profile is how one person works, not shared live state.
