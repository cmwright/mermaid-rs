# mermaid-rs

A pure Rust implementation of the [Mermaid](https://mermaid.js.org/) diagram renderer. Converts `.mmd` files to SVG without requiring a browser, Node.js, or Puppeteer.

> **Status:** Early stage — flowchart diagrams are supported. Other diagram types are planned.

## Quick Start

```bash
# Build
cargo build --release

# Render a diagram
cargo run -- -i diagram.mmd -o diagram.svg

# Or use the binary directly
./target/release/mermaid -i diagram.mmd -o diagram.svg

# Read from stdin
cat diagram.mmd | cargo run -- -i - -o output.svg
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
└── mermaid-core/      # Core library
    └── src/
        ├── parser/    # PEG grammar + parsing → AST
        ├── ast/       # Diagram data structures
        ├── layout/    # Hierarchical graph layout (dagre/petgraph)
        └── render/    # SVG generation + themes
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

### Not Yet Implemented

Sequence, class, state, ER, Gantt, and pie diagrams.

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
