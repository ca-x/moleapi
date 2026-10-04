import { useLanguage } from "../../shared/i18n";
import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, native, token } from "../../shared/api";
import type { AuthStatus } from "../../shared/types";

export function useAuth() {
  useLanguage();
  const client = useQueryClient();
  const [authenticated, setAuthenticated] = useState(!!token() || native);
  const [accountId, setAccountId] = useState(() =>
    native ? "local" : sessionStorage.getItem("moleapi_username") || "session",
  );
  const status = useQuery({
    queryKey: ["auth-status"],
    queryFn: () => api<AuthStatus>("/api/auth/status"),
  });
  useEffect(() => {
    const fail = () => {
      if (!native) {
        sessionStorage.removeItem("moleapi_token");
        setAuthenticated(false);
      }
    };
    window.addEventListener("moleapi:unauthorized", fail);
    return () => window.removeEventListener("moleapi:unauthorized", fail);
  }, []);
  const login = () => {
    const account = native
      ? "local"
      : sessionStorage.getItem("moleapi_username") || "session";
    setAccountId(account);
    setAuthenticated(true);
    void client.invalidateQueries({ queryKey: ["workspaces", account] });
    void client.invalidateQueries({ queryKey: ["auth-status"] });
  };
  const endSession = async () => {
    try {
      await api("/api/auth/logout", "POST");
    } catch {
      /* An expired session can still be removed locally. */
    }
    sessionStorage.removeItem("moleapi_token");
    setAuthenticated(false);
    client.removeQueries({ queryKey: ["workspaces"] });
  };
  return {
    authenticated,
    setAuthenticated,
    accountId,
    status,
    login,
    endSession,
  };
}
