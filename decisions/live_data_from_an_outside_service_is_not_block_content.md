# Live data from an outside service is not block content

A block that shows an outside service's data (the boat app's sandboxes) keeps in its content only
what is ours: which account it shows and how it filters. Each viewer reads the service's data with
their own credentials, and the last answer is kept in that viewer's view state (synced, but the
viewer's own), so the editor opens with it at once and says how old it is, or that it is offline.

## Why

- The service's own access decides what each viewer sees: two people viewing one block see what
  their own accounts may see, and a viewer with no account sees nothing.
- Sharing the block shares no machine's name, address or state with someone the service never
  showed them to.
- The data changes on the service's schedule, not by anyone's edit: kept in content, every poll
  would be a commit in the block's history, and two viewers polling at once would conflict over a
  value neither of them wrote.

## Rejected

- Keeping the list in content so a viewer without a key still sees it: that is the leak above.
- Keeping it in device-local storage only: apps have none (guides/apps/host_services.md), and the
  view state already belongs to the viewer and goes with them to their other devices.
- Writing the view state on every poll: it is written only when the list changed or the kept copy
  is five minutes old, enough to say its age truthfully without a commit every fifteen seconds.
