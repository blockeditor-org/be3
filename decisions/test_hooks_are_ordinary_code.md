# Test hooks are ordinary code, not a feature

be-host-stack has no `testing` feature. What tests of the stack and of what runs on it need
from it (`be_host_stack::testing`: the harness, the block types a test serves in process, and
the hooks the stack keeps for them, such as waiting on its state, held clocks and
`assets_for_testing`) is compiled into every native build, inert unless a test calls it. The
server a test signs in against is not in it: `be_host_server::testing::Server`, beneath
be-host-stack, which only tests link, so nothing a host ships links be-server
(decisions/no_crate_splits_inside_dependency_chains.md).

## Why

- buck builds each library once per platform, with the features of that platform's
  `cargo test` plan (guides/buck2.md, "Features merge per plan"). be-host-testing, a
  dev-dependency of be-host, be-host-runtime and be-tool-cli, turned `testing` on, so it was on
  in every native build, shipped be-host included: a feature always on was a pretence that the
  code it gated did not ship.
- Turning it off for shipped builds alone takes a configuration under which every test builds
  its whole graph, a second compile of everything a test reaches, which
  decisions/verify_compiles_one_configuration_per_target.md rejects.
- A second, test-only build of be-host-stack beside the usual one would put two copies of the
  stack's per-process globals in one test of be-host, one through be-host and one through
  be-host-testing.

## Rejected

- Keeping the `testing` feature: always on, it gated nothing.
- A test configuration in which shipped builds plan without dev-dependencies: a second compile
  of every target a test reaches.
- The server crates as optional dependencies of be-host-stack behind the feature: be-server in
  every native be-host.
