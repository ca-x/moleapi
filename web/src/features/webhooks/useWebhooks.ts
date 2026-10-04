import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, native, saveFile } from "../../shared/api";
import { errorCopy, type ErrorCopy } from "../../shared/i18n/errors";
import { translateCopy } from "../../shared/i18n";
import type { ExportResult } from "../../shared/types";
import type { Receiver, Batch, Listener } from "./model";
export function useWebhooks(workspace: string) {
  const client = useQueryClient();
  const [selected, select] = useState("");
  const [privateView, setPrivateView] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorCopy>("");
  const live = useRef(true);
  useEffect(() => { live.current = true; return () => { live.current = false; }; }, []);
  const receivers = useQuery({ queryKey: ["webhooks", workspace], queryFn: () => api<Receiver[]>(`/api/workspaces/${workspace}/webhooks`), refetchInterval: 2000 });
  const receiver = receivers.data?.find(i => i.id === selected);
  const captures = useQuery({ queryKey: ["webhook-captures", workspace, selected, privateView], queryFn: () => api<Batch>(`/api/webhooks/${selected}/captures?include_secrets=${privateView}`), enabled: !!receiver, refetchInterval: 2000 });
  const listener = useQuery({ queryKey: ["webhook-listener"], queryFn: () => api<Listener>("/api/webhooks/listener"), enabled: native, refetchInterval: 2000 });
  async function refresh() {
    await Promise.all([client.invalidateQueries({ queryKey: ["webhooks", workspace] }), client.invalidateQueries({ queryKey: ["webhook-captures", workspace] }), client.invalidateQueries({ queryKey: ["webhook-listener"] })]);
  }
  async function act(action: () => Promise<void>) {
    if (busy) return;
    setBusy(true); setError("");
    try { await action(); await refresh(); } catch (e) { if (live.current) setError(errorCopy(e)); }
    finally { if (live.current) setBusy(false); }
  }
  function choose(id: string) { select(id); setPrivateView(false); setError(""); }
  return { receivers, receiver, captures, listener, privateView, setPrivateView, busy, error: translateCopy(error), act, choose,
    create: (name: string) => act(async () => { const i = await api<Receiver>(`/api/workspaces/${workspace}/webhooks`, "POST", { name }); choose(i.id); }),
    update: (i: Receiver) => act(async () => { await api(`/api/webhooks/${i.id}`, "PUT", { name: i.name, active: i.active, response: i.response, expected_revision: i.revision }); }),
    remove: (i: Receiver) => act(async () => { await api(`/api/webhooks/${i.id}`, "DELETE", { expected_revision: i.revision }); choose(""); }),
    clear: (i: Receiver) => act(async () => { await api(`/api/webhooks/${i.id}/captures`, "DELETE"); }),
    export: (i: Receiver) => act(async () => { await saveFile(await api<ExportResult>(`/api/webhooks/${i.id}/export`, "POST", { include_secrets: privateView })); }),
    start: (host: string, port: number) => act(async () => { await api("/api/webhooks/listener/start", "POST", { host, port }); }),
    stop: () => act(async () => { await api("/api/webhooks/listener/stop", "POST", {}); }),
  };
}
