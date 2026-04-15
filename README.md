# mermaid-rs

A pure Rust implementation of the [Mermaid](https://mermaid.js.org/) diagram renderer. Converts `.mmd` files to SVG without requiring a browser, Node.js, or Puppeteer.

## Installation

```bash
cargo install --git https://github.com/cmwright/mermaid-rs mermaid-cli
```

This installs the `mmrs` binary.

## Quick Start

```bash
# Render a diagram
mmrs -i diagram.mmd -o diagram.svg

# Read from stdin
cat diagram.mmd | mmrs -i - -o output.svg
```

### Building from source

```bash
cargo build --release
./target/release/mmrs -i diagram.mmd -o diagram.svg
```

### CLI Options

```
-i, --input <FILE>    Input .mmd file (use `-` for stdin)
-o, --output <FILE>   Output SVG path (default: output.svg)
-t, --theme <THEME>   Theme: default, dark, forest, neutral
-w, --width <PX>      Max width in pixels
    --font <FILE>     Custom TTF/OTF font file
-v                    Verbose logging
```

## Project Structure

```
crates/
├── mermaid-cli/       # CLI binary (`mermaid`)
├── mermaid-core/      # Core library
│   └── src/
│       ├── parser/    # PEG grammar + parsing → AST
│       ├── ast/       # Diagram data structures
│       ├── layout/    # Hierarchical graph layout (dagre/petgraph)
│       └── render/    # SVG generation + themes
└── mermaid-wasm/      # WASM bindings for browser

live-editor/           # Browser-based live editor (Rsbuild + TypeScript)
```

**Pipeline:** Source `.mmd` → Parse (PEG) → AST → Layout (dagre) → SVG

## What's Supported

### Flowcharts

- **Directions:** TB, BT, LR, RL
- **Node shapes:** rectangle, rounded, stadium, subroutine, cylinder, circle, double circle, diamond, hexagon, asymmetric, trapezoid, parallelogram
- **Edges:** solid/dotted/thick arrows and lines, with labels
- **Styling:** `classDef`, `style` overrides, inline `:::class` shorthand
- **Subgraphs:** nested, with custom direction
- **Themes:** default, dark, forest, neutral

### Other Diagrams

- **Sequence diagrams** - actors, messages, notes, loops, activations
- **Gantt charts** - sections, tasks, dates
- **Pie charts** - segments with values
- **Git graphs** - branches, commits, merges
- **Mind maps** - hierarchical tree diagrams
- **Architecture diagrams** - service groups and connections
- **State diagrams** - states and transitions

### Not Yet Implemented

Class diagrams, ER diagrams.

## Live Editor

**[Try it in your browser](https://cmwright.github.io/mermaid-rs/)**

A browser-based live editor similar to the Mermaid.js live editor is included. Edit diagrams in real-time with live preview, theme switching, and example templates.

```bash
# Start development server with hot reload
make dev-live-editor

# Build for production
make build-live-editor

# Serve production build
make serve-live-editor
```

Features:

- **Live preview** with 300ms debounced rendering
- **Monaco Editor** with syntax highlighting
- **4 themes** (default, dark, forest, neutral)
- **12+ examples** (flowcharts, sequence diagrams, gantt, pie, etc.)
- **URL sharing** - diagrams are encoded in the URL hash
- **Export** - download or copy SVG

## Testing

```bash
make test          # Run unit/integration tests
make test-svgs     # Render all fixture examples to SVG
make clean-svgs    # Clean test output
```

Test fixtures live in `tests/integration/fixtures/`.

### Examples Comparison

Generate an HTML page that compares mermaid-rs output against mermaid.js side by side:

```bash
make test-examples
open target/examples-comparison.html
```

This renders a set of example diagrams and produces a three-column view (input, mermaid-rs, mermaid.js) so you can visually compare the results in your browser.
