# mermaid-cli

`mmrs` — a command-line tool that renders [Mermaid](https://mermaid.js.org/) diagrams to SVG, PNG, or ASCII. Pure Rust; no browser or Node.js required.

```bash
cargo install mermaid-rs-cli
```

```bash
mmrs -i diagram.mmd -o diagram.svg
cat diagram.mmd | mmrs -i - -o out.svg
```

## Options

```
-i, --input <FILE>    Input .mmd file (use `-` for stdin)
-o, --output <FILE>   Output path (default: output.svg)
-t, --theme <THEME>   default | dark | forest | neutral
-w, --width <PX>      Max width in pixels
    --font <FILE>     Custom TTF/OTF font
-v                    Verbose logging
```

See the [project repository](https://github.com/cmwright/mermaid-rs) for diagram support status.

## License

Dual-licensed under MIT or Apache-2.0.
