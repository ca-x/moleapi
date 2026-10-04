import { useLanguage } from "../../shared/i18n";
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../../shared/api";
import type { ApiResponse, HistoryEntry } from "../../shared/types";
export function useHistory(
  authenticated: boolean,
  selectedId: string,
  active: boolean,
  accountId = "local",
) {
  useLanguage();
  const history = useQuery({
    queryKey: ["history", selectedId, accountId],
    queryFn: () => api<HistoryEntry[]>(`/api/workspaces/${selectedId}/history`),
    enabled: authenticated && !!selectedId && active,
  });
  const [selection, setSelection] = useState<{
    accountId: string;
    workspaceId: string;
    response: ApiResponse;
  } | null>(null);
  const setHistoryResponse = (response: ApiResponse | null) =>
    setSelection(
      response ? { accountId, workspaceId: selectedId, response } : null,
    );
  return {
    history,
    setHistoryResponse,
    historyResponse:
      selection?.accountId === accountId &&
      selection?.workspaceId === selectedId
        ? selection.response
        : null,
  };
}
