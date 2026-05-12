# mermaid-core

Pure-Rust [Mermaid](https://mermaid.js.org/) diagram parser and renderer. Converts Mermaid source to SVG (and optionally PNG / ASCII) without a browser, Node.js, or Puppeteer.

```toml
[dependencies]
mermaid-core = "0.1"
```

```rust
use mermaid_core::render_svg;

let svg = render_svg("graph TD; A-->B; B-->C")?;
```

## Features

- `png` (default) — PNG rasterization via `resvg`
- `ascii` (default) — terminal/ASCII output
- `wasm` — `wasm-bindgen`-compatible build

Disable defaults for a slim SVG-only build:

```toml
mermaid-core = { version = "0.1", default-features = false }
```

## Supported diagrams

Flowchart, sequence, gantt, and class diagrams. See the [project repository](https://github.com/cmwright/mermaid-rs) for the full status matrix.

## License

Dual-licensed under MIT or Apache-2.0.
