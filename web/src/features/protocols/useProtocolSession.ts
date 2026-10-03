import { useEffect, useRef, useState } from "react";
import { useQuery, CancelledError } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "../../shared/api";
import { safeMessage } from "../../shared/model";
import type { RequestSpec } from "../../shared/types";
import type { useLocalVariables } from "../variables/useLocalVariables";
import type { useWorkspace } from "../workspaces/useWorkspace";
import { appendEvents } from "./events";
import type {
  EventBatch,
  ProtocolEvent,
  ProtocolSession,
  SendMessage,
} from "./types";
interface Active {
  account: string;
  workspace: string;
  request: string;
  protocol: string;
  generation: number;
  session: ProtocolSession;
}
export function useProtocolSession(
  workspace: ReturnType<typeof useWorkspace>,
  request: RequestSpec | undefined,
  locals: ReturnType<typeof useLocalVariables>,
  authenticated: boolean,
) {
  const kind = request?.protocol?.kind || "http";
  const identity = JSON.stringify([
    authenticated,
    workspace.accountId,
    workspace.draft?.id,
    workspace.draft?.data.active_environment_id,
    request?.id,
    kind,
  ]);
  const identityRef = useRef(identity);
  const mounted = useRef(true);
  const generation = useRef(0);
  if (identityRef.current !== identity) {
    identityRef.current = identity;
    generation.current++;
  }
  const scopeKey = () =>
    JSON.stringify([identityRef.current, generation.current]);
  const current = (origin: string, epoch: number) =>
    mounted.current &&
    identityRef.current === origin &&
    generation.current === epoch;
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      generation.current++;
    };
  }, []);
  const [active, setActive] = useState<Active | null>(null);
  const [retained, setRetained] = useState<{
    events: ProtocolEvent[];
    dropped: number;
  }>({ events: [], dropped: 0 });
  const [error, setError] = useState<{
    identity: string;
    message: string;
  } | null>(null);
  const [connecting, setConnecting] = useState<string[]>([]);
  const [sendingFor, setSendingFor] = useState<string[]>([]);
  const operations = useRef(new Set<string>());
  const sends = useRef(new Set<string>());
  const cursor = useRef(0);
  const live =
    authenticated &&
    active?.generation === generation.current &&
    active.account === workspace.accountId &&
    active.workspace === workspace.draft?.id &&
    active.request === request?.id &&
    active.protocol === kind
      ? active
      : null;
  const sessionId = live?.session.id;
  const sessionRef = useRef(sessionId);
  sessionRef.current = sessionId;
  useEffect(() => {
    setRetained({ events: [], dropped: 0 });
    setError(null);
    cursor.current = 0;
    return () => {
      if (sessionId)
        void api(
          `/api/sessions/${encodeURIComponent(sessionId)}/close`,
          "POST",
          {},
        ).catch(() => {});
    };
  }, [identity, sessionId]);
  const poll = useQuery({
    queryKey: ["protocol-session", workspace.accountId, sessionId],
    enabled: authenticated && !!sessionId,
    retry: false,
    queryFn: async () => {
      const epoch = generation.current;
      const check = () => {
        if (!current(identity, epoch) || sessionRef.current !== sessionId)
          throw new CancelledError({ silent: true });
      };
      check();
      // Read state first so terminal-state events are available in the final batch.
      const session = await api<ProtocolSession>(
        `/api/sessions/${encodeURIComponent(sessionId!)}`,
      );
      check();
      const batch = await api<EventBatch>(
        `/api/sessions/${encodeURIComponent(sessionId!)}/events?after=${cursor.current}`,
      );
      check();
      return { session, batch, identity, generation: epoch };
    },
    refetchInterval: (query) =>
      query.state.data &&
      ["closed", "error"].includes(query.state.data.session.state)
        ? false
        : 750,
  });
  useEffect(() => {
    const update = poll.data;
    if (
      !update ||
      !current(update.identity, update.generation) ||
      update.session.id !== sessionRef.current
    )
      return;
    cursor.current = update.batch.next_cursor;
    setRetained((previous) => {
      const next = appendEvents(previous.events, update.batch);
      return { events: next.events, dropped: previous.dropped + next.dropped };
    });
    setActive((previous) =>
      previous?.session.id === update.session.id
        ? { ...previous, session: update.session }
        : previous,
    );
  }, [poll.data]);
  async function connect() {
    const draft = workspace.draft;
    if (
      !authenticated ||
      !mounted.current ||
      !draft ||
      !request ||
      kind === "http" ||
      operations.current.has(scopeKey()) ||
      (live && ["connecting", "open"].includes(live.session.state))
    )
      return;
    const origin = identity;
    const epoch = generation.current;
    const ticket = scopeKey();
    const snapshot = structuredClone(request);
    const collection = draft.data.collections.find((c) =>
      c.requests.some((r) => r.id === snapshot.id),
    )?.id;
    const environment = draft.data.active_environment_id;
    const localValues = locals.values(draft, collection, environment);
    operations.current.add(ticket);
    setConnecting([...operations.current]);
    setError(null);
    try {
      if (workspace.dirty && !(await workspace.save(true))) return;
      if (!current(origin, epoch)) return;
      const session = await api<ProtocolSession>("/api/sessions", "POST", {
        workspace_id: draft.id,
        request: snapshot,
        environment_id: environment,
        ...(localValues.length ? { locals: localValues } : {}),
      });
      if (!current(origin, epoch)) {
        void api(
          `/api/sessions/${encodeURIComponent(session.id)}/close`,
          "POST",
          {},
        ).catch(() => {});
        return;
      }
      locals.apply(
        draft,
        collection,
        environment,
        session.variable_updates || [],
      );
      setActive({
        account: workspace.accountId,
        workspace: draft.id,
        request: snapshot.id,
        protocol: kind,
        generation: epoch,
        session,
      });
    } catch (caught) {
      if (current(origin, epoch))
        setError({ identity: origin, message: safeMessage(caught) });
    } finally {
      operations.current.delete(ticket);
      if (mounted.current) setConnecting([...operations.current]);
    }
  }
  async function close() {
    if (!sessionId) return;
    const origin = identity;
    const epoch = generation.current;
    try {
      const session = await api<ProtocolSession>(
        `/api/sessions/${encodeURIComponent(sessionId)}/close`,
        "POST",
        {},
      );
      if (current(origin, epoch) && sessionRef.current === sessionId)
        setActive((previous) =>
          previous?.session.id === session.id
            ? { ...previous, session }
            : previous,
        );
      if (current(origin, epoch) && sessionRef.current === sessionId)
        void poll.refetch();
    } catch (caught) {
      if (current(origin, epoch) && sessionRef.current === sessionId)
        setError({ identity: origin, message: safeMessage(caught) });
    }
  }
  async function send(message: SendMessage) {
    if (
      !sessionId ||
      live?.session.state !== "open" ||
      sends.current.has(scopeKey())
    )
      return;
    const origin = identity;
    const epoch = generation.current;
    const ticket = scopeKey();
    sends.current.add(ticket);
    setSendingFor([...sends.current]);
    try {
      await api(
        `/api/sessions/${encodeURIComponent(sessionId)}/send`,
        "POST",
        message,
      );
      if (current(origin, epoch) && sessionRef.current === sessionId)
        void poll.refetch();
    } catch (caught) {
      if (current(origin, epoch) && sessionRef.current === sessionId)
        toast.error(safeMessage(caught));
    } finally {
      sends.current.delete(ticket);
      if (mounted.current) setSendingFor([...sends.current]);
    }
  }
  return {
    session: live?.session || null,
    events: live ? retained.events : [],
    dropped: live ? retained.dropped : 0,
    error:
      error?.identity === identity
        ? error.message
        : sessionId && poll.error
          ? safeMessage(poll.error)
          : "",
    busy: connecting.includes(scopeKey()),
    sending: sendingFor.includes(scopeKey()),
    connect,
    close,
    send,
  };
}
