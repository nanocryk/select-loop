# `select_loop!`

Provides a `select_loop!` macro allowing to loop awaiting for multiple streams
and/or futures, and is aimed to be used instead of using tokio's `select!` macro
inside a loop, which may cause issues with [cancelation safety].

Select loops are a nice design pattern to write async tasks continuously awaiting
for events/messages while managing an internal state without mutexes, since only
one event/message is processed at a time.

[cancelation safety]:
    https://docs.rs/tokio/1.25.0/tokio/macro.select.html#cancellation-safety

## Usage

The body of the `select_loop!` macro looks like a match statement with
additional syntax.

- `F my_future => |item| ...,` allows to await for a Future. Once the Future
resolves the branch is executed.
- `S my_stream => |item| ...,` allows to await
for a Stream. Each time the Stream outputs an item the branch is executed.

> The macro takes ownership of the Future/Stream and requires it to be
> Unpin.

Additionally it is possible to hook on various stages of the select loop:

- `@before => ...` is executed just before any branch is executed, but after one
  of the futures or stream has outputed an item.
- `@after => ...` is executed just after any branch is executed.
- `@exhausted => ...` is executed when all futures and streams are exhausted.
  The value returned by the last `@exhausted` branch is returned by the
  `select_loop!` (it corresponds to a `break ...;` inside the macro expansion).

As the macro expands into a Rust `loop`, `break` and `continue` can be used like
with normal loops. `break` followed by a value can even be used as long as all
`break` return values of the same type, and that at least one `@exhausted`
branch exists and the last one returns a value of the same type. If you have
multiple nested loops, you can refer to the select loop using the `'select_loop`
lifetime.

## Examples

Examples are available [here](https://github.com/nanocryk/select-loop/tree/main/examples)