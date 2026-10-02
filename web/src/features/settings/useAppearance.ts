import { useEffect, useState } from "react";
export function useAppearance() {
  const [dark, setDark] = useState(
    () => localStorage.getItem("moleapi_theme") === "dark",
  );
  useEffect(() => {
    localStorage.setItem("moleapi_theme", dark ? "dark" : "light");
  }, [dark]);
  return { dark, setDark };
}
