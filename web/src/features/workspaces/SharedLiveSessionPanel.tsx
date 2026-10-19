import { useEffect, useRef, useState } from "react";
import { Button, Callout, Flex, Heading, Text, TextArea } from "@radix-ui/themes";
import { api, token } from "../../shared/api";
import { t, useLanguage } from "../../shared/i18n";
import { liveError } from "../../shared/i18n/errors";
import type { RequestSpec } from "../../shared/types";
import { Choice, Field } from "../../shared/ui";
import { appendEvents } from "../protocols/events";
import SessionEventPane from "../protocols/SessionEventPane";
import type { EventBatch, ProtocolEvent, ProtocolSession, SendMessage } from "../protocols/types";

export default function SharedLiveSessionPanel({ base, request, revision, environmentId, dirty, saving, conflict, dark }: {
  base: string;
  request: RequestSpec;
  revision: number;
  environmentId: string | null;
  dirty: boolean;
  saving: boolean;
  conflict: boolean;
  dark: boolean;
}) {
  useLanguage();
  const [session, setSession] = useState<ProtocolSession | null>(null);
  const [events, setEvents] = useState<ProtocolEvent[]>([]);
  const [dropped, setDropped] = useState(0);
  const [mode, setMode] = useState<"text" | "binary" | "ping">("text");
  const [payload, setPayload] = useState("");
  const [busy, setBusy] = useState(false);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<ReturnType<typeof liveError>>("");
  const cursor = useRef(0);
  const generation = useRef(0);
  const credential = useRef(token());
  const sessionId = session?.id;
  const stateRef = useRef(session?.state);
  stateRef.current = session?.state;
  function fail(cause: unknown) {
    setError(liveError(cause));
    if (cause && typeof cause === "object" && "status" in cause && [401, 403, 404, 409].includes(Number(cause.status))) {
      setSession(null);
      setEvents([]);
      cursor.current = 0;
    }
  }
  useEffect(() => () => { generation.current++; }, []);
  useEffect(() => {
    const current = ++generation.current;
    if (!sessionId) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const path = `${base}/sessions/${encodeURIComponent(sessionId)}`;
    async function poll() {
      try {
        const summary = await api<ProtocolSession>(path);
        const batch = await api<EventBatch>(`${path}/events?after=${cursor.current}`);
        if (current !== generation.current) return;
        cursor.current = batch.next_cursor;
        setEvents((previous) => {
          const next = appendEvents(previous, batch);
          setDropped((count) => count + next.dropped);
          return next.events;
        });
        setSession(summary);
        if (summary.state === "open" || summary.state === "connecting") timer = setTimeout(poll, 750);
      } catch (cause) {
        if (current === generation.current) fail(cause);
      }
    }
    void poll();
    return () => {
      generation.current++;
      clearTimeout(timer);
      if (credential.current && (stateRef.current === "open" || stateRef.current === "connecting")) void fetch(`${path}/close`, { method: "POST", headers: { Authorization: `Bearer ${credential.current}` } }).catch(() => {});
    };
  }, [base, sessionId]);
  async function connect() {
    if (busy || session || dirty || saving || conflict) return;
    const current = generation.current;
    setBusy(true); setError(""); setEvents([]); setDropped(0); cursor.current = 0;
    try {
      const opened = await api<ProtocolSession>(`${base}/sessions`, "POST", { request_id: request.id, environment_id: environmentId, expected_revision: revision });
      if (current === generation.current) setSession(opened);
      else if (credential.current) void fetch(`${base}/sessions/${encodeURIComponent(opened.id)}/close`, { method: "POST", headers: { Authorization: `Bearer ${credential.current}` } }).catch(() => {});
    } catch (cause) { if (current === generation.current) fail(cause); }
    finally { if (current === generation.current) setBusy(false); }
  }
  async function close() {
    if (!sessionId || busy) return;
    setBusy(true);
    try { const closed = await api<ProtocolSession>(`${base}/sessions/${encodeURIComponent(sessionId)}/close`, "POST", {}); setSession(closed); }
    catch (cause) { fail(cause); }
    finally { setBusy(false); }
  }
  async function reset() {
    if (!sessionId || busy) return;
    setBusy(true); setError("");
    try { await api(`${base}/sessions/${encodeURIComponent(sessionId)}`, "DELETE"); setSession(null); setEvents([]); setDropped(0); cursor.current = 0; }
    catch (cause) { fail(cause); }
    finally { setBusy(false); }
  }
  async function send() {
    if (!sessionId || session?.state !== "open" || sending) return;
    const message: SendMessage = mode === "text" ? { kind: "text", text: payload } : { kind: mode, base64: payload };
    setSending(true); setError("");
    try { await api(`${base}/sessions/${encodeURIComponent(sessionId)}/send`, "POST", message); }
    catch (cause) { fail(cause); }
    finally { setSending(false); }
  }
  return <section className="shared-live-session" aria-label={t("共享实时会话")}>
    <Flex direction="column" gap="3">
      <Heading size="3">{t("实时会话")}</Heading>
      {error && <Callout.Root color="red" role="alert"><Callout.Text>{error}</Callout.Text></Callout.Root>}
      {!session && <><Button loading={busy} disabled={busy || dirty || saving || conflict} onClick={() => void connect()}>{t("连接")}</Button>{dirty && <Text size="1" color="gray">{t("先保存修改，再连接实时会话。")}</Text>}</>}
      {session && <><Flex gap="2" align="center" wrap="wrap"><Text role="status" size="2">{t("状态")}: {({ connecting: t("连接中"), open: t("已连接"), closed: t("已关闭"), error: t("连接失败") })[session.state]}</Text>{(session.state === "open" || session.state === "connecting") ? <Button variant="soft" color="gray" loading={busy} onClick={() => void close()}>{t("断开")}</Button> : <Button variant="soft" disabled={busy || dirty || saving || conflict} onClick={() => void reset()}>{t("重新连接")}</Button>}</Flex>
        {session.reason && <Text size="2" color="gray">{session.reason}</Text>}
        {session.protocol === "websocket" && session.state === "open" && <Flex direction="column" gap="2"><Choice label={t("消息类型")} value={mode} onChange={(value) => setMode(value as typeof mode)} options={[{ value: "text", label: t("文本") }, { value: "binary", label: "Base64" }, { value: "ping", label: "Ping Base64" }]} /><Field label={mode === "text" ? t("消息正文") : "Base64"}><TextArea value={payload} onChange={(event) => setPayload(event.target.value)} /></Field><Button loading={sending} disabled={sending} onClick={() => void send()}>{t("发送消息")}</Button></Flex>}
        <SessionEventPane events={events} dropped={dropped} dark={dark} sessionId={sessionId} protocolLabel={session.protocol.toUpperCase()} />
      </>}
    </Flex>
  </section>;
}
