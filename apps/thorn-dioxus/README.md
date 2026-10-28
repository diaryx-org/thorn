# Thorn

The drawing app: `crates/thorn-dioxus`'s editor in a window drawn by
Dioxus's native renderer (`dioxus-native` 0.8.0-alpha.3, Blitz on wgpu, no
webview), as Leaf is.

```sh
cargo run -p thorn-dioxus-app -- drawing.svg   # a drawing; a new one without
```

| | |
|---|---|
| `main.rs` | the window, launched on the path given |
| `app.rs` | the page: the bar (the drawing's name, New, Open, Save), the editor under it, the window's size and appearance, and the keys — unit's first, then ⌘N, ⌘O, ⌘S and ⇧⌘S, then the canvas's |
| `keys.rs` | winit's keys as Dioxus's |

What the archived Swift app did that this does not yet is the task
[What the Dioxus app does not do yet](../../docs/tasks/what-the-dioxus-app-does-not-do-yet.md).
