import { createElement } from "react";
import i18next from "i18next";
import { initReactI18next, useTranslation } from "react-i18next";
import en from "./en.json";
import zhCN from "./zh-CN.json";
import monacoZhCN from "./monaco-zh-CN.json";

export type Language = "zh-CN" | "en";
export const LANGUAGE_STORAGE_KEY = "moleapi_language";
export function resolveLanguage(saved: string | null, browserLanguage: string): Language {
  if (saved === "zh-CN" || saved === "en") return saved;
  return /^zh(?:-|$)/i.test(browserLanguage) ? "zh-CN" : "en";
}
function initialLanguage(): Language {
  let saved: string | null = null;
  try { saved = localStorage.getItem(LANGUAGE_STORAGE_KEY); } catch { /* Storage can be denied. */ }
  return resolveLanguage(saved, typeof navigator === "undefined" ? "en" : navigator.language);
}
export const i18n = i18next.createInstance();
void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, "zh-CN": { translation: zhCN, monaco: monacoZhCN } },
  lng: initialLanguage(), fallbackLng: "en", supportedLngs: ["zh-CN", "en"],
  keySeparator: false, nsSeparator: false, initAsync: false,
  interpolation: { escapeValue: false }, react: { useSuspense: false },
});
function updateDocument(language: string) {
  if (typeof document !== "undefined") {
    document.documentElement.lang = language;
    document.title = language === "zh-CN" ? "MoleAPI · API 工作台" : "MoleAPI · API Workbench";
  }
}
i18n.on("languageChanged", updateDocument);
updateDocument(i18n.language);
export function t(key: string, values?: Record<string, unknown>): string {
  return String(i18n.t(key, values));
}
async function syncNativeLanguage(language: Language): Promise<void> {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    if (i18n.language !== language) return;
    await invoke("set_language", { language });
  } catch { /* A missing tray or bridge must not prevent UI language changes. */ }
}
void syncNativeLanguage(i18n.language as Language);
export async function setLanguage(language: Language): Promise<void> {
  try { localStorage.setItem(LANGUAGE_STORAGE_KEY, language); } catch { /* Keep an in-memory preference. */ }
  await i18n.changeLanguage(language);
  await syncNativeLanguage(language);
}
export function useLanguage() {
  useTranslation(undefined, { i18n });
  return { language: i18n.language as Language, setLanguage };
}

/** Keep app-owned dialog copy as a key until rendering, so open dialogs switch too. */
export type LocalizedCopy = { key: string; values?: Record<string, unknown> };
export function message(key: string, values?: Record<string, unknown>): LocalizedCopy {
  return { key, values };
}
export function translateCopy(copy: string | LocalizedCopy | undefined): string {
  return typeof copy === "string" ? copy : copy ? t(copy.key, copy.values) : "";
}
function LocalizedText({ copy }: { copy: LocalizedCopy }) {
  useLanguage();
  return translateCopy(copy);
}
/** React content lets an already visible Sonner toast follow live language changes. */
export function liveTranslation(key: string, values?: Record<string, unknown>) {
  return createElement(LocalizedText, { copy: message(key, values) });
}
