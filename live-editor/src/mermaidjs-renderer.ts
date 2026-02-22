import mermaid from 'mermaid';
import type { RenderResult, Theme } from './wasm-loader';

let initialized = false;
let renderCounter = 0;

export function initMermaidJs(): void {
  if (initialized) return;
  mermaid.initialize({
    startOnLoad: false,
    suppressErrorRendering: true,
    theme: 'default',
  });
  initialized = true;
}

export async function renderMermaidJs(source: string, theme: Theme): Promise<RenderResult> {
  initMermaidJs();

  // mermaid.js uses the same theme names: default, dark, forest, neutral
  mermaid.initialize({ theme, startOnLoad: false });

  const id = `mermaid-js-render-${renderCounter++}`;

  try {
    const { svg } = await mermaid.render(id, source);
    return { svg, error: null };
  } catch (error) {
    // mermaid.render creates a temp element with the id; clean it up on error
    const el = document.getElementById('d' + id);
    el?.remove();
    return {
      svg: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}
