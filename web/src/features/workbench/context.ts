import { createContext, useContext } from "react";
import type { useWorkbenchController } from "./useWorkbenchController";
export const WorkbenchContext = createContext<ReturnType<
  typeof useWorkbenchController
> | null>(null);
export function useWorkbench() {
  const state = useContext(WorkbenchContext);
  if (!state) throw new Error("Workbench provider is missing");
  return state;
}
