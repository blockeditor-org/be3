# The filter's blur is Vello's Gaussian blur

The blur in beui's filter pass (`Context::apply_filter`, `Group::Filter`) is vello_gpu's Gaussian blur filter layer. `Filter::blur` is the blur's standard deviation σ in points, as CSS `blur()` takes it, and the blur clamps at the filter's region (`EdgeMode::Duplicate`). What the filter covers is drawn into a scene texture; Vello blurs that texture, drawn as an image, into a second one; beui's combine pass (`combine.wgsl`) composes the result over the target within the damage, applying the contrast and the colour vision matrix, which vello_gpu 0.4 does not offer.

## Why

Vello halves the image until little blur remains and finishes with a kernel of at most 13 taps, keeping exact count of the variance each level adds: the blur equals the σ asked for at every radius, continuously, and costs about as much for a large radius as a small one. Its reach is 3σ, which bounds how far the damage is widened. The dual Kawase chain it replaced snapped when its level count changed, which showed as steps while the radius moved.

The blur is rendered over the whole target each frame a filter is drawn. Under a clip, vello_gpu 0.4's filter layer takes nothing from outside the clip's pixels, so a blur clipped to the damage lost the light from just beside it.

## Rejected

- **The dual Kawase chain** (`blur.wgsl`): see above.
- **Vello filters for the contrast and the colour vision matrix**: vello_common 0.4 lists them but leaves them unimplemented.
