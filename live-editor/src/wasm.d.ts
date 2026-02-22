// Type declarations for WASM module
declare module '/wasm/mermaid_wasm.js' {
  export function render_svg(source: string): { svg: string | null; error: string | null };
  export function render_svg_with_theme(source: string, theme: string): { svg: string | null; error: string | null };
  export function get_supported_themes(): string[];
  export default function init(): Promise<void>;
}
