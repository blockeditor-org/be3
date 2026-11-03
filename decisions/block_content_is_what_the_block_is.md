# A block's content is what the block is, not what an editor makes of it

A block type stores the block's own substance, and an editor's derived view of it stays in the editor. Where something derived is worth keeping with the block, it is kept as a header the editor fills in through an ordinary operation: the image block keeps the file's bytes and a header saying what decoding them found, or why it failed, which the editor writes the first time it draws the image (guides/adding_a_block.md, Define the model).

## Why

Nothing but the app then needs a decoder, and no other client has to decode the block to know its shape.
