# The sequence keeps its fragments in chunks, not a sum tree

`be-lib-sequence` keeps fragments in chunks with visible counts, plus an index
from each fragment's first position to its chunk, so finding a position or a
visible offset does not walk the whole document.

## Rejected

- A sum tree, like Zed's, which would make lookups logarithmic: chunks are
  enough until profiling says otherwise.
