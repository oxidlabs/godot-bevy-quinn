# Godot + Bevy + Quinnet Chat Demo

A minimal chat demo that connects a Godot client to a Bevy (Rust) server over Quinnet.

## Prerequisites
- Rust toolchain (stable)
- Godot 4.x

## Quick Start

1) Run the server (from the `rust` folder):

```bash
cd rust
cargo build
cargo run --bin server
```

2) Run a client in Godot:
- Open this repository as a Godot project (the `project.godot` is at the repo root).
- Open the scene `test.tscn`.
- Press Play.

You can open multiple Godot editor instances (or export a build) and run several clients at once to chat between them.

## Notes
- The server must be running before launching clients.
- The chat scene is `test.tscn`; make sure you run this scene when testing.
- Messages are sent when you submit text in the input (mapped to `ui_text_submit`).

## Folder Structure
- `rust/` — Bevy/Quinnet server and GDNative binding library
- `rust/src/server.rs` — Server binary entrypoint (`cargo run --bin server`)
- `test.tscn` — Godot client scene to run