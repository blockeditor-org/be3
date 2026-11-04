# A block's access is the highest grant on the path up

A block's access is the higher of its own grant (or authorship) and every
ancestor's (`level_below` in `be-block-graph`), so a grant lower down never
reduces the View, Edit or Manage a person inherits. Only `Manage` shares and
changes settings; `Edit` no longer shares. Authors and organization
administrators get Manage. Key work (sealing, rotation) needs only Edit.

## Why

- Nobody can be locked out of what they manage.
- Nobody can keep personal files in someone else's workspace out of its
  owner's reach.
- Key work only carries out access already decided, so it needs no more than
  Edit.

## Rejected

- The nearest grant wins: a lower grant could then take inherited access away.
