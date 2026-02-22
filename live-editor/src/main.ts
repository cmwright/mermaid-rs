import './styles/index.css';
import Split from 'split.js';
import { createEditor, getEditorValue, setEditorValue } from './editor';
import { createPreview, updatePreview, downloadSvg, copySvgToClipboard } from './preview';
import { createToolbar, updateThemeSelector } from './toolbar';
import { getInitialState, saveStateToUrl, type EditorState } from './state';
import { renderDiagram, type Theme, loadWasm } from './wasm-loader';
import { renderMermaidJs } from './mermaidjs-renderer';
import type { Example } from './examples/templates';

class LiveEditor {
  private state: EditorState;
  private isRendering = false;
  private pendingRender = false;
  private lastCode = '';
  private toolbarContainer: HTMLElement | null = null;
  private errorPanel: HTMLElement | null = null;

  constructor() {
    this.state = getInitialState();
    this.init();
  }

  private async init(): Promise<void> {
    const app = document.getElementById('app');
    if (!app) {
      console.error('App container not found');
      return;
    }

    app.innerHTML = `
      <div class="flex flex-col h-full">
        <div id="toolbar" class="toolbar"></div>
        <div id="split-container" class="split-container flex-1">
          <div id="editor-panel" class="panel">
            <div id="editor-container" class="editor-container"></div>
            <div id="error-panel" class="error-panel font-hack text-xs"></div>
          </div>
          <div id="preview-panel" class="panel">
            <div id="mermaid-rs-preview" class="preview-pane">
              <div class="preview-label">mermaid-rs</div>
              <div id="mermaid-rs-container" class="preview-container"></div>
            </div>
            <div id="mermaid-js-preview" class="preview-pane">
              <div class="preview-label">mermaid.js</div>
              <div id="mermaid-js-container" class="preview-container"></div>
            </div>
            <div id="loading-overlay" class="loading-overlay">
              <div class="spinner"></div>
            </div>
          </div>
        </div>
      </div>
    `;

    const toolbarEl = document.getElementById('toolbar')!;
    const editorContainer = document.getElementById('editor-container')!;
    const mermaidRsContainer = document.getElementById('mermaid-rs-container')!;
    const mermaidJsContainer = document.getElementById('mermaid-js-container')!;
    this.errorPanel = document.getElementById('error-panel');
    this.toolbarContainer = toolbarEl;

    // Horizontal split: editor | preview
    Split(['#editor-panel', '#preview-panel'], {
      sizes: [50, 50],
      minSize: [300, 300],
      gutterSize: 8,
      direction: 'horizontal',
    });

    // Vertical split inside preview: mermaid-rs on top | mermaid-js on bottom
    Split(['#mermaid-rs-preview', '#mermaid-js-preview'], {
      sizes: [50, 50],
      minSize: [100, 100],
      gutterSize: 8,
      direction: 'vertical',
    });

    createToolbar({
      container: toolbarEl,
      currentTheme: this.state.theme as Theme,
      onThemeChange: (theme) => this.handleThemeChange(theme),
      onExampleSelect: (example) => this.handleExampleSelect(example),
      onDownload: () => downloadSvg(),
      onCopy: () => copySvgToClipboard(),
    });

    createEditor({
      container: editorContainer,
      initialValue: this.state.code,
      onChange: (value: string) => this.handleCodeChange(value),
    });

    createPreview({
      mermaidRsContainer,
      mermaidJsContainer,
    });

    try {
      await loadWasm();
      this.render();
    } catch (error) {
      this.showError(error instanceof Error ? error.message : 'Failed to initialize');
    }
  }

  private handleCodeChange(code: string): void {
    this.state.code = code;
    saveStateToUrl(this.state);
    this.render();
  }

  private handleThemeChange(theme: Theme): void {
    this.state.theme = theme;
    this.lastCode = '';
    saveStateToUrl(this.state);
    this.render();
  }

  private handleExampleSelect(example: Example): void {
    setEditorValue(example.code);
    this.state.code = example.code;
    saveStateToUrl(this.state);
    this.render();
  }

  private async render(): Promise<void> {
    if (this.isRendering) {
      this.pendingRender = true;
      return;
    }

    const code = getEditorValue();
    if (code === this.lastCode && !this.pendingRender) {
      return;
    }

    this.isRendering = true;
    this.pendingRender = false;
    this.showLoading(true);

    const theme = this.state.theme as Theme;

    try {
      // Run both renderers in parallel
      const [rsResult, jsResult] = await Promise.all([
        renderDiagram(code, theme),
        renderMermaidJs(code, theme),
      ]);

      // Update mermaid-rs preview
      if (rsResult.error) {
        this.showError(rsResult.error);
        updatePreview('mermaid-rs', null, theme);
      } else if (rsResult.svg) {
        this.showError(null);
        updatePreview('mermaid-rs', rsResult.svg, theme);
      }

      // Update mermaid-js preview
      if (jsResult.error) {
        updatePreview('mermaid-js', null, theme);
      } else if (jsResult.svg) {
        updatePreview('mermaid-js', jsResult.svg, theme);
      }

      if (!rsResult.error) {
        this.lastCode = code;
      }
    } catch (error) {
      this.showError(error instanceof Error ? error.message : 'Unknown error');
    } finally {
      this.isRendering = false;
      this.showLoading(false);

      if (this.pendingRender) {
        this.pendingRender = false;
        setTimeout(() => this.render(), 0);
      }
    }
  }

  private showError(error: string | null): void {
    if (this.errorPanel) {
      if (error) {
        this.errorPanel.textContent = error;
        this.errorPanel.style.display = 'block';
      } else {
        this.errorPanel.style.display = 'none';
      }
    }
  }

  private showLoading(show: boolean): void {
    const overlay = document.getElementById('loading-overlay');
    if (overlay) {
      overlay.classList.toggle('active', show);
    }
  }
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => {
    new LiveEditor();
  });
} else {
  new LiveEditor();
}
