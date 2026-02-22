import * as monaco from 'monaco-editor';

export interface EditorConfig {
  container: HTMLElement;
  initialValue: string;
  onChange: (value: string) => void;
}

let editor: monaco.editor.IStandaloneCodeEditor | null = null;

export function createEditor(config: EditorConfig): monaco.editor.IStandaloneCodeEditor {
  // Configure Monaco environment
  (self as any).MonacoEnvironment = {
    getWorkerUrl: function (_moduleId: string, label: string) {
      if (label === 'json') {
        return '/monaco-editor/esm/vs/language/json/json.worker.js';
      }
      if (label === 'css' || label === 'scss' || label === 'less') {
        return '/monaco-editor/esm/vs/language/css/css.worker.js';
      }
      if (label === 'html' || label === 'handlebars' || label === 'razor') {
        return '/monaco-editor/esm/vs/language/html/html.worker.js';
      }
      if (label === 'typescript' || label === 'javascript') {
        return '/monaco-editor/esm/vs/language/typescript/ts.worker.js';
      }
      return '/monaco-editor/esm/vs/editor/editor.worker.js';
    }
  };

  // Register mermaid language
  monaco.languages.register({ id: 'mermaid' });
  
  monaco.languages.setMonarchTokensProvider('mermaid', {
    tokenizer: {
      root: [
        [new RegExp('(flowchart|sequenceDiagram|gantt|pie|gitGraph|mindmap|architecture-beta|stateDiagram|stateDiagram-v2)\\b'), 'keyword'],
        [new RegExp('(TB|BT|LR|RL|TD)\\b'), 'keyword.direction'],
        [new RegExp('(-->>|->>|--x|->x|\\)\\)|\\)\\)|--\\>|->|==>>|==>|\\.->)'), 'arrow'],
        [new RegExp('[\\[\\(\\{<]'), 'bracket.open'],
        [new RegExp('[\\]\\)\\}>]'), 'bracket.close'],
        [new RegExp('\\|[^\\|]*\\|'), 'string.edge-label'],
        [new RegExp('"[^"]*"'), 'string'],
        [new RegExp("'[^']*'"), 'string'],
        [new RegExp('#.*$'), 'comment'],
        [new RegExp('(%%%).*'), 'comment.block'],
        [new RegExp('(subgraph|end|classDef|style|linkStyle|click|call|href)\\b'), 'keyword.control'],
        [new RegExp('(section|title)\\b'), 'keyword.section'],
        [new RegExp('[a-zA-Z][a-zA-Z0-9_-]*'), 'identifier'],
        [new RegExp('[\\-:\\|]'), 'delimiter'],
      ],
    },
  });

  // Define theme
  monaco.editor.defineTheme('mermaid-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'keyword', foreground: '569CD6', fontStyle: 'bold' },
      { token: 'keyword.direction', foreground: '4EC9B0' },
      { token: 'keyword.control', foreground: 'C586C0' },
      { token: 'keyword.section', foreground: '9CDCFE' },
      { token: 'arrow', foreground: 'CE9178' },
      { token: 'bracket.open', foreground: 'FFD700' },
      { token: 'bracket.close', foreground: 'FFD700' },
      { token: 'string', foreground: 'CE9178' },
      { token: 'string.edge-label', foreground: 'DCDCAA' },
      { token: 'comment', foreground: '6A9955' },
      { token: 'comment.block', foreground: '6A9955' },
      { token: 'identifier', foreground: 'D4D4D4' },
      { token: 'delimiter', foreground: 'D4D4D4' },
    ],
    colors: {
      'editor.background': '#1e1e1e',
      'editor.foreground': '#d4d4d4',
      'editorLineNumber.foreground': '#858585',
      'editor.selectionBackground': '#264f78',
      'editor.lineHighlightBackground': '#2d2d30',
    },
  });

  editor = monaco.editor.create(config.container, {
    value: config.initialValue,
    language: 'mermaid',
    theme: 'mermaid-dark',
    fontFamily: 'Hack, monospace',
    fontSize: 14,
    minimap: { enabled: false },
    automaticLayout: true,
    scrollBeyondLastLine: false,
    lineNumbers: 'on',
    renderLineHighlight: 'all',
    quickSuggestions: false,
    parameterHints: { enabled: false },
    suggestOnTriggerCharacters: false,
    acceptSuggestionOnEnter: 'off',
    tabCompletion: 'off',
    wordBasedSuggestions: 'off',
    folding: true,
    foldingStrategy: 'indentation',
  });

  // Handle changes with debouncing done in main.ts
  editor.onDidChangeModelContent(() => {
    const value = editor!.getValue();
    config.onChange(value);
  });

  return editor;
}

export function getEditor(): monaco.editor.IStandaloneCodeEditor | null {
  return editor;
}

export function setEditorValue(value: string): void {
  if (editor) {
    editor.setValue(value);
  }
}

export function getEditorValue(): string {
  return editor ? editor.getValue() : '';
}

export function disposeEditor(): void {
  if (editor) {
    editor.dispose();
    editor = null;
  }
}
