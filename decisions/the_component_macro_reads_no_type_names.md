# The component macro reads no type names

`#[component]` and `view!` (crates/beui-macros) do not recognise prop types by name. Every
setter is generic over what it is passed and asks for `IntoPropValue<P, M>` of the prop's
declared type `P`; each prop type says in `beui-tree` (`prop_value.rs`) what it takes, with one
impl keyed on the type. Children written between a tag's tags go to one hidden `__children`
setter, wrapped by the syntax they are written in, and the slot type says which shapes it takes
(`TakesChildren`, `TakesBlock`, `TakesClosure`, `TakesUnitClosure`, `TakesWith`). A prop is
required unless its type has an `AbsentProp` value or it has a `#[prop(default = ..)]`;
`#[prop(required)]` is the one manual tag.

The macro reads a type in three places, all structural or syntactic:

- A prop whose last written type argument is one of the component's own type parameters, named
  by no other prop, takes only what decides that parameter (`IntoExactProp`, `ExactTakes`).
  The rule looks through `Option`, the one name it reads besides the check below: an
  `Option<RenderFn<H, C>>` takes what an exact `RenderFn<H, C>` does, as `Some`.
- A prop typed by a bare type parameter takes its value as it is, and cannot be the children
  slot.
- `Render<(), C>` and `RenderFn<(), C>` are rejected: a render handed nothing is a `Build<C>` or
  `BuildFn<C>`.

The macro reads exactly two type names: `Option`, so an exact prop works inside one, and
`()` as the handle of `Render`/`RenderFn`, to reject it. Each one is here only because no trait
can express it: whether a type argument is one of the component's own parameters is decided
before traits are resolved, and a rejected type has no trait to fail. Before adding a third name,
look for a trait that answers the question instead, and record the new name here if none can.

## Why

- A new prop type, in any crate, works in a tag by implementing traits, not by teaching the
  macro its name; a type alias or a re-export under another name behaves the same.
- One impl per prop type, keyed on the type and never on the value's type, means a value a
  prop does not take has one candidate, and rustc reports the obligation that failed under it
  (`a u32 cannot be a child of this component`) with that trait's message, instead of listing
  every type `IntoPropValue` knows.
- Requiredness as typestate makes a missing prop a compile error at the tag, naming the prop.

## Rejected

- Recognising prop types by name (`Prop`, `Callback`, `RenderFn`, ...) to pick a setter: it
  broke on aliases and re-exports, and every new prop type needed a macro change.
- A `#[prop(exact)]` tag for props whose type comes from what is passed: the structural rule
  already finds them, and a tag would be one more thing a component author must remember.
- Defaults chosen by alias (a type alias that carries its default): an alias is invisible to
  trait resolution, so it needed the name sniffing this decision removes.
- `Render<(), C>` for a render handed nothing: a closure child naming a value could not be
  told apart from one that ignores `()`, so `Build<C>` and `BuildFn<C>` are the only way to
  write one.
- A render prop also taking a closure that ignores its handle (`|| ..`): the two closure impls
  made a wrong closure ambiguous, and rustc then reported neither failure. Such a closure is
  written `|_| ..`; between tags, `|| { .. }` still reaches a render slot by its syntax.
