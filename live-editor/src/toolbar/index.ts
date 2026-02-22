import { examples, type Example } from '../examples/templates';
import type { Theme } from '../wasm-loader';

export interface ToolbarConfig {
  container: HTMLElement;
  currentTheme: Theme;
  onThemeChange: (theme: Theme) => void;
  onExampleSelect: (example: Example) => void;
  onDownload: () => void;
  onCopy: () => void;
}

export function createToolbar(config: ToolbarConfig): void {
  config.container.innerHTML = `
    <select id="example-select" class="toolbar-select">
      <option value="">Load Example...</option>
      ${examples.map(e => `<option value="${escapeHtml(e.name)}">${escapeHtml(e.name)}</option>`).join('')}
    </select>
    
    <select id="theme-select" class="toolbar-select">
      <option value="default" ${config.currentTheme === 'default' ? 'selected' : ''}>Default Theme</option>
      <option value="dark" ${config.currentTheme === 'dark' ? 'selected' : ''}>Dark Theme</option>
      <option value="forest" ${config.currentTheme === 'forest' ? 'selected' : ''}>Forest Theme</option>
      <option value="neutral" ${config.currentTheme === 'neutral' ? 'selected' : ''}>Neutral Theme</option>
    </select>
    
    <div class="flex-1"></div>
    
    <button id="copy-btn" class="toolbar-btn secondary" title="Copy SVG to clipboard">
      Copy SVG
    </button>
    
    <button id="download-btn" class="toolbar-btn" title="Download SVG">
      Download SVG
    </button>
  `;
  
  // Example selector
  const exampleSelect = config.container.querySelector('#example-select') as HTMLSelectElement;
  exampleSelect.addEventListener('change', () => {
    const selectedName = exampleSelect.value;
    if (selectedName) {
      const example = examples.find(e => e.name === selectedName);
      if (example) {
        config.onExampleSelect(example);
      }
      exampleSelect.value = ''; // Reset to placeholder
    }
  });
  
  // Theme selector
  const themeSelect = config.container.querySelector('#theme-select') as HTMLSelectElement;
  themeSelect.addEventListener('change', () => {
    config.onThemeChange(themeSelect.value as Theme);
  });
  
  // Download button
  const downloadBtn = config.container.querySelector('#download-btn') as HTMLButtonElement;
  downloadBtn.addEventListener('click', config.onDownload);
  
  // Copy button
  const copyBtn = config.container.querySelector('#copy-btn') as HTMLButtonElement;
  copyBtn.addEventListener('click', config.onCopy);
}

export function updateThemeSelector(container: HTMLElement, theme: Theme): void {
  const themeSelect = container.querySelector('#theme-select') as HTMLSelectElement;
  if (themeSelect) {
    themeSelect.value = theme;
  }
}

function escapeHtml(text: string): string {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}
