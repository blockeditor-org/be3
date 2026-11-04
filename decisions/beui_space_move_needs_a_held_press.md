# Space starts a move or resize only while the press it answers is held

In beui-layer-space, a program's `WindowRequest::BeginMove` or `BeginResize` starts a carry or resize only while a press the program was given is still held (any window's press; for a laser, while its trigger holds that window). A request that arrives after that press was let go is dropped.

## Why

Wayland ties an interactive move to the press that started it, and an app's request reaches be-host a frame or two after the press it answers. Starting a carry from a request that arrives after the click was released would leave the window stuck to the cursor until the next click.

## Rejected

- Honour every `BeginMove`/`BeginResize` whenever it arrives: a late request leaves the window stuck to the cursor until the next click.
