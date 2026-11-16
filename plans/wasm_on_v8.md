# A guest thread's own instances

Depends on: none.

Every wasm guest runs on V8 and nothing is precompiled (decisions/guests_run_on_v8.md). What is
left:

1. **A thread's own instances.** Every guest thread runs on an isolate of its own, so each
   thread V8 starts instantiates its app and every library it links again: the module is
   compiled once and shared (`Compiled`), but instantiating and linking the dylink imports repeat
   per thread, and code V8 optimises for one isolate is shared through the module while each
   isolate's own wrappers and feedback are not. Measure what a thread's start costs in a host test (an app
   that spawns threads, such as the text editor), and if it shows, make a thread's instance
   cheaper: link against the trimmed export list (`keep_exports`) once per module rather than
   per instance, or keep a warm isolate per guest to hand a new thread. Done: a thread's start
   measured, and either made cheaper with the before and after in decisions/guests_run_on_v8.md
   or recorded there as not worth it.
