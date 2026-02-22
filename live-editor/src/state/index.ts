const STORAGE_KEY = 'mermaid-rs-editor-state';

export interface EditorState {
  code: string;
  theme: string;
}

export function getInitialState(): EditorState {
  // First try URL hash
  if (window.location.hash.length > 1) {
    try {
      const decoded = atob(window.location.hash.slice(1));
      const parsed = JSON.parse(decoded);
      if (parsed.code && typeof parsed.code === 'string') {
        return {
          code: parsed.code,
          theme: parsed.theme || 'default',
        };
      }
    } catch {
      // Invalid hash, ignore
    }
  }
  
  // Fallback to default
  return {
    code: `flowchart TD
    A[Start] --> B{Is it?}
    B -->|Yes| C[OK]
    C --> D[Rethink]
    D --> B
    B -->|No| E[End]`,
    theme: 'default',
  };
}

export function saveStateToUrl(state: EditorState): void {
  const data = JSON.stringify({
    code: state.code,
    theme: state.theme,
  });
  const encoded = btoa(data);
  window.location.hash = encoded;
}

export function debounce<T extends (...args: any[]) => void>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout>;
  return (...args: Parameters<T>) => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => fn(...args), delay);
  };
}
