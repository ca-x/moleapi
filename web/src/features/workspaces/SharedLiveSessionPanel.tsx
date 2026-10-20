import { useEffect, useRef, useState } from "react";
import { Button, Callout, Flex, Heading, Tabs, Text, TextArea } from "@radix-ui/themes";
import { api, token } from "../../shared/api";
import { t, useLanguage } from "../../shared/i18n";
import { liveError } from "../../shared/i18n/errors";
import type { RequestSpec } from "../../shared/types";
import { Choice, Field } from "../../shared/ui";
import { appendEvents } from "../protocols/events";
import type { SaveSharedResultDataset } from "./SharedResultDatasetDialog";
import SharedDataSessionControls from "./SharedDataSessionControls";
import SharedTcpSessionControls from "./SharedTcpSessionControls";
import SessionEventPane from "../protocols/SessionEventPane";
import SessionHandshake from "../protocols/SessionHandshake";
import type { EventBatch, ProtocolEvent, ProtocolSession, SendMessage } from "../protocols/types";

export default function SharedLiveSessionPanel({ base, request, revision, environmentId, dirty, saving, conflict, dark, isCurrent, onSaveResultDataset }: {
  base: string;
  request: RequestSpec;
  revision: number;
  environmentId: string | null;
  dirty: boolean;
  saving: boolean;
  conflict: boolean;
  dark: boolean;
  isCurrent: () => boolean;
  onSaveResultDataset: SaveSharedResultDataset;
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
  const mounted = useRef(true);
  const admission = useRef({ isCurrent, dirty, saving, conflict }); admission.current = { isCurrent, dirty, saving, conflict };
  const currentContext = () => mounted.current && admission.current.isCurrent() && !admission.current.dirty && !admission.current.conflict;
  const allowed = () => currentContext() && !admission.current.saving;
  const cursor = useRef(0);
  const generation = useRef(0);
  const credential = useRef(token());
  const sessionId = session?.id;
  function fail(cause: unknown) {
    setError(liveError(cause));
    if (cause && typeof cause === "object" && "status" in cause && [401, 403, 404, 409].includes(Number(cause.status))) {
      setSession(null);
      setEvents([]);
      cursor.current = 0;
    }
  }
  function clearPrivate() { setSession(null); setEvents([]); setDropped(0); cursor.current = 0; }
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; generation.current++; }; }, []);
  useEffect(() => {
    const current = ++generation.current;
    if (!sessionId) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const path = `${base}/sessions/${encodeURIComponent(sessionId)}`;
    async function poll() {
      if (current !== generation.current) return;
      if (!currentContext()) { clearPrivate(); return; }
      try {
        const summary = await api<ProtocolSession>(path);
        if (current !== generation.current) return;
        if (!currentContext()) { clearPrivate(); return; }
        const batch = await api<EventBatch>(`${path}/events?after=${cursor.current}`);
        if (current !== generation.current) return;
        if (!currentContext()) { clearPrivate(); return; }
        cursor.current = batch.next_cursor;
        setEvents((previous) => {
          const next = appendEvents(previous, batch);
          setDropped((count) => count + next.dropped);
          return next.events;
        });
        setSession(summary);
        if (summary.state === "open" || summary.state === "connecting") timer = setTimeout(poll, 750);
      } catch (cause) {
        if (current === generation.current) { if (currentContext()) fail(cause); else clearPrivate(); }
      }
    }
    void poll();
    return () => {
      generation.current++;
      clearTimeout(timer);
      if (credential.current) void fetch(`${path}/close`, { method: "POST", headers: { Authorization: `Bearer ${credential.current}` } }).catch(() => {});
    };
  }, [base, sessionId]);
  async function connect() {
    if (busy || session || !allowed()) return;
    const current = generation.current;
    setBusy(true); setError(""); setEvents([]); setDropped(0); cursor.current = 0;
    try {
      const opened = await api<ProtocolSession>(`${base}/sessions`, "POST", { request_id: request.id, environment_id: environmentId, expected_revision: revision });
      if (current === generation.current && allowed()) setSession(opened);
      else if (credential.current) void fetch(`${base}/sessions/${encodeURIComponent(opened.id)}/close`, { method: "POST", headers: { Authorization: `Bearer ${credential.current}` } }).catch(() => {});
    } catch (cause) { if (current === generation.current && allowed()) fail(cause); }
    finally { if (mounted.current && current === generation.current) setBusy(false); }
  }
  async function close() {
    if (!sessionId || busy || !allowed()) return;
    const current = generation.current;
    setBusy(true);
    try { const closed = await api<ProtocolSession>(`${base}/sessions/${encodeURIComponent(sessionId)}/close`, "POST", {}); if (current === generation.current && allowed()) setSession(closed); }
    catch (cause) { if (current === generation.current && allowed()) fail(cause); }
    finally { if (mounted.current && current === generation.current) setBusy(false); }
  }
  async function reset() {
    if (!sessionId || busy || !allowed()) return;
    const current = generation.current;
    setBusy(true); setError("");
    try { await api(`${base}/sessions/${encodeURIComponent(sessionId)}`, "DELETE"); if (current === generation.current && allowed()) clearPrivate(); }
    catch (cause) { if (current === generation.current && allowed()) fail(cause); }
    finally { if (mounted.current && current === generation.current) setBusy(false); }
  }
  async function sendData(message: SendMessage) {
    if (!sessionId || session?.state !== "open" || sending || !allowed()) return false;
    const current = generation.current;
    setSending(true); setError("");
    try { await api(`${base}/sessions/${encodeURIComponent(sessionId)}/send`, "POST", message); return current === generation.current && allowed(); }
    catch (cause) { if (current === generation.current && allowed()) fail(cause); return false; }
    finally { if (mounted.current && current === generation.current) setSending(false); }
  }
  async function send() {
    const message: SendMessage = mode === "text" ? { kind: "text", text: payload } : { kind: mode, base64: payload };
    await sendData(message);
  }
  return <section className="shared-live-session" aria-label={t("共享实时会话")}>
    <Flex direction="column" gap="3">
      <Heading size="3">{t("实时会话")}</Heading>
      {error && <Callout.Root color="red" role="alert"><Callout.Text>{error}</Callout.Text></Callout.Root>}
      {!session && <><Button loading={busy} disabled={busy || dirty || saving || conflict} onClick={() => void connect()}>{t("连接")}</Button>{dirty && <Text size="1" color="gray">{t("先保存修改，再连接实时会话。")}</Text>}</>}
      {session && <><Flex gap="2" align="center" wrap="wrap"><Text role="status" size="2">{t("状态")}: {({ connecting: t("连接中"), open: t("已连接"), closed: t("已关闭"), error: t("连接失败") })[session.state]}</Text>{(session.state === "open" || session.state === "connecting") ? <Button variant="soft" color="gray" loading={busy} onClick={() => void close()}>{t("断开")}</Button> : <Button variant="soft" disabled={busy || dirty || saving || conflict} onClick={() => void reset()}>{t("重新连接")}</Button>}</Flex>
        {session.reason && <Text size="2" color="gray">{session.reason}</Text>}
        {session.protocol === "websocket" && session.state === "open" && <Flex direction="column" gap="2"><Choice label={t("消息类型")} value={mode} onChange={(value) => setMode(value as typeof mode)} options={[{ value: "text", label: t("文本") }, { value: "binary", label: "Base64" }, { value: "ping", label: "Ping Base64" }]} /><Field label={mode === "text" ? t("消息正文") : "Base64"}><TextArea value={payload} onChange={(event) => setPayload(event.target.value)} /></Field><Button loading={sending} disabled={sending} onClick={() => void send()}>{t("发送消息")}</Button></Flex>}
        {session.protocol === "tcp" && request.protocol?.kind === "tcp" && <SharedTcpSessionControls key={sessionId} session={session} initial={request.protocol.message} dark={dark} sending={sending} isCurrent={allowed} send={sendData} />}
        {session.protocol === "data" && request.protocol?.kind === "data" ? <SharedDataSessionControls key={sessionId} sessionId={sessionId!} config={request.protocol} events={events} dropped={dropped} open={session.state === "open"} sending={sending} dark={dark} send={sendData} onSaveResultDataset={onSaveResultDataset} /> : <Tabs.Root defaultValue="events"><Tabs.List><Tabs.Trigger value="events">{t("消息")}</Tabs.Trigger>{session.protocol !== "tcp" && <Tabs.Trigger value="handshake">{t("握手")}</Tabs.Trigger>}</Tabs.List><Tabs.Content value="events"><SessionEventPane events={events} dropped={dropped} dark={dark} sessionId={sessionId} protocolLabel={session.protocol.toUpperCase()} /></Tabs.Content><Tabs.Content value="handshake"><SessionHandshake session={session} /></Tabs.Content></Tabs.Root>}
      </>}
    </Flex>
  </section>;
}
