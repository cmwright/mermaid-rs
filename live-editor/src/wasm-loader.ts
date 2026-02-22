// WASM Module loader and renderer

export interface RenderResult {
  svg: string | null;
  error: string | null;
}

export type Theme = 'default' | 'dark' | 'forest' | 'neutral';

let wasmModule: any = null;
let isLoading = false;
let loadPromise: Promise<void> | null = null;

export async function loadWasm(): Promise<void> {
  if (wasmModule) return;
  if (isLoading && loadPromise) return loadPromise;
  
  isLoading = true;
  loadPromise = loadWasmInternal();
  return loadPromise;
}

async function loadWasmInternal(): Promise<void> {
  try {
    console.log('Loading WASM module...');

    // Resolve the WASM JS path relative to the document so it works
    // both at the root (local dev) and under a subpath (GitHub Pages).
    const wasmUrl = new URL('wasm/mermaid_wasm.js', document.baseURI).href;

    // Use webpackIgnore to bypass rspack's module resolution and use native
    // browser import(). This lets the browser load the wasm-pack generated JS
    // directly, where import.meta.url resolves correctly for the WASM binary.
    const wasm = await import(/* webpackIgnore: true */ wasmUrl);

    console.log('WASM JS module imported:', Object.keys(wasm));

    // Initialize the WASM module — __wbg_init uses import.meta.url to find
    // mermaid_wasm_bg.wasm relative to the JS file, which works correctly
    // since the browser loaded it from /wasm/mermaid_wasm.js
    await wasm.default();

    wasmModule = wasm;
    console.log('WASM module loaded successfully');
  } catch (error) {
    console.error('Failed to load WASM module:', error);
    throw new Error('Failed to initialize WASM module. Please refresh the page.');
  }
}

export async function renderDiagram(source: string, theme: Theme = 'default'): Promise<RenderResult> {
  console.log('Rendering diagram with theme:', theme);
  await loadWasm();
  
  if (!wasmModule) {
    console.error('WASM module not loaded');
    return { svg: null, error: 'WASM module not loaded' };
  }
  
  try {
    console.log('Calling render_svg_with_theme...');
    const result = wasmModule.render_svg_with_theme(source, theme);
    console.log('Render result:', result);
    
    // Handle the result from wasm-bindgen
    if (result && typeof result === 'object') {
      const svg = result.svg;
      const error = result.error;
      
      return {
        svg: svg || null,
        error: error || null,
      };
    }
    
    // Fallback for string return (old API)
    if (typeof result === 'string') {
      console.log('Got string result');
      return { svg: result, error: null };
    }
    
    console.error('Unexpected result type:', typeof result, result);
    return { svg: null, error: 'Unexpected response from WASM module' };
  } catch (error) {
    console.error('Error rendering diagram:', error);
    return {
      svg: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

export function getSupportedThemes(): string[] {
  if (!wasmModule) {
    return ['default', 'dark', 'forest', 'neutral'];
  }
  
  try {
    return wasmModule.get_supported_themes();
  } catch {
    return ['default', 'dark', 'forest', 'neutral'];
  }
}
