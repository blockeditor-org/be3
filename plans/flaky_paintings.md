# Flaky paintings

Depends on: none.

A painting test's result is cached on its code alone (guides/buck2.md, Tests), so a test that
paints nondeterministically keeps one painting until its code changes, and the painting flips
only when the test reruns. Nothing yet says such a test is flaky.

When CI accepts a changed painting, run that test once more, uncached, and report it as flaky
when the two runs disagree. Do it only inside `./scripts/ci`'s one buck2 invocation and only if
it stays cheap: the rerun is an action of its own, taking the `painting_compare` output among
its inputs, so it runs once per painting change and its result is cached like any other. It
only reports; the test's own action and its cached painting stay as they are.
