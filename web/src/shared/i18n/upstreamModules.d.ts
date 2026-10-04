declare module "monaco-editor/esm/vs/nls.js" {
  export function localize(key: string, message: string, ...values: unknown[]): string;
  export function localize2(key: string, message: string, ...values: unknown[]): { readonly value: string; readonly original: string };
  export function localizeFixedLabel(label: string): string;
  export function onLanguageChange(callback: () => void): { dispose(): void };
}
