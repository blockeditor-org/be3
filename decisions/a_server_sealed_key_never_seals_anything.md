# A key the server seals is accepted only to read the organization's settings

The server seals the organization settings block's scope key to each member
itself (`ServerKey::seal_settings_to`, on `GetKeys`). Clients accept a
server-sealed key for that block alone and only to read it (`KeyBook::accepts`);
such a key is never trusted, so it never seals another block. Administrators
get a trusted copy from another administrator through `tend`.

## Why

Members need the key without an administrator online, but a key that comes
from the server is never trusted, so it is limited to reading this one block
(be-block-client
`a_key_the_server_labels_as_the_organizations_settings_never_seals_another_block`).
