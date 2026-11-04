# Settings about the data follow the block, settings about the viewer follow the viewer

Each settings kind declares `FOLLOWS` (`Block` or `Viewer`). Tag definitions,
prefixes and the owning organization's data policies resolve from where the
block lives; theme, trust and the viewer's own offline choice resolve from the
viewer, whatever they are looking at, and skip the block layer. When an
owner's and a viewer's organization policies conflict, the stricter wins.
Resolution order: organization policy, organization defaults, the nearest
block with settings, the user's every-device layer, the user's this-device
layer; a `Policy` kind fixes the fields it sets for later layers, a `Default`
kind lets later layers win.

## Why

It is the settings plan's first principle: a setting resolves from what it
is about, the data or the person looking at it.
