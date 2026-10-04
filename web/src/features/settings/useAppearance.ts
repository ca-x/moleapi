import { useLanguage } from "../../shared/i18n";
import { useEffect, useState } from "react";
export function useAppearance() {
  useLanguage();
  const [dark, setDark] = useState(
    () => { try { return localStorage.getItem("moleapi_theme") === "dark"; } catch { return false; } },
  );
  useEffect(() => {
    try { localStorage.setItem("moleapi_theme", dark ? "dark" : "light"); } catch { /* Appearance remains available when storage is denied. */ }
  }, [dark]);
  return { dark, setDark };
}
