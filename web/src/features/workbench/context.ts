import { useLanguage } from "../../shared/i18n";
import { createContext, useContext } from "react";
import type { useWorkbenchController } from "./useWorkbenchController";
export const WorkbenchContext = createContext<ReturnType<
  typeof useWorkbenchController
> | null>(null);
export function useWorkbench() {
  useLanguage();
  const state = useContext(WorkbenchContext);
  if (!state) throw new Error("Workbench provider is missing");
  return state;
}
