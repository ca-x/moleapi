import { useEffect } from "react";
/** Keyboard navigation is immediate; occasional pointer dialogs may use subtle motion. */
export function useInputModality() {
  useEffect(() => {
    const markKeyboard = () => {
      document.documentElement.dataset.inputModality = "keyboard";
    };
    const markPointer = () => {
      document.documentElement.dataset.inputModality = "pointer";
    };
    document.addEventListener("keydown", markKeyboard, true);
    document.addEventListener("pointerdown", markPointer, true);
    return () => {
      document.removeEventListener("keydown", markKeyboard, true);
      document.removeEventListener("pointerdown", markPointer, true);
    };
  }, []);
}
