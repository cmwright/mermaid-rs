import type { Theme } from '../wasm-loader';

export type PreviewTarget = 'mermaid-rs' | 'mermaid-js';

export interface PreviewConfig {
  mermaidRsContainer: HTMLElement;
  mermaidJsContainer: HTMLElement;
}

let containers: Record<PreviewTarget, HTMLElement | null> = {
  'mermaid-rs': null,
  'mermaid-js': null,
};
let currentSvg: string | null = null;

export function createPreview(config: PreviewConfig): void {
  containers['mermaid-rs'] = config.mermaidRsContainer;
  containers['mermaid-js'] = config.mermaidJsContainer;

  const placeholder = '<div class="text-gray-500 text-sm">Start typing to see preview...</div>';
  containers['mermaid-rs']!.innerHTML = placeholder;
  containers['mermaid-js']!.innerHTML = placeholder;
}

export function updatePreview(target: PreviewTarget, svg: string | null, theme: Theme): void {
  const container = containers[target];
  if (!container) return;

  if (!svg) {
    container.innerHTML = '<div class="text-gray-500 text-sm">No diagram to display</div>';
    if (target === 'mermaid-rs') currentSvg = null;
    return;
  }

  if (target === 'mermaid-rs') currentSvg = svg;

  const bgColors: Record<Theme, string> = {
    default: '#ffffff',
    dark: '#1f2020',
    forest: '#ffffff',
    neutral: '#ffffff',
  };

  container.style.backgroundColor = bgColors[theme] || '#ffffff';
  container.innerHTML = svg;
}

export function showError(error: string | null): void {
  // Errors are shown in the editor error panel, not in preview containers
  if (!error) return;
}

export function getCurrentSvg(): string | null {
  return currentSvg;
}

export function downloadSvg(): void {
  if (!currentSvg) return;

  const blob = new Blob([currentSvg], { type: 'image/svg+xml' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = 'diagram.svg';
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}

export function copySvgToClipboard(): void {
  if (!currentSvg) return;

  navigator.clipboard.writeText(currentSvg).then(() => {
    showToast('SVG copied to clipboard!');
  }).catch(() => {
    showToast('Failed to copy SVG', 'error');
  });
}

function showToast(message: string, type: 'success' | 'error' = 'success'): void {
  const toast = document.createElement('div');
  toast.className = `fixed bottom-4 right-4 px-4 py-2 rounded text-sm font-hack z-50 transition-opacity duration-300 ${
    type === 'success' ? 'bg-green-600 text-white' : 'bg-red-600 text-white'
  }`;
  toast.textContent = message;
  document.body.appendChild(toast);

  setTimeout(() => {
    toast.style.opacity = '0';
    setTimeout(() => {
      document.body.removeChild(toast);
    }, 300);
  }, 2000);
}
