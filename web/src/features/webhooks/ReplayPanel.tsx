import { useEffect, useRef, useState } from "react";
import { Button, Callout, Checkbox, Flex, Heading, Text, TextArea, TextField } from "@radix-ui/themes";
import { api } from "../../shared/api";
import { errorCopy, type ErrorCopy } from "../../shared/i18n/errors";
import { PairEditor, Choice } from "../../shared/ui";
import { t, useLanguage, translateCopy } from "../../shared/i18n";
import type { ApiResponse } from "../../shared/types";
import { ResponsePane } from "../requests/ResponsePane";
import { replayRows, type Capture } from "./model";
export function ReplayPanel({ inbox, capture, dark }: { inbox: string; capture: Capture; dark: boolean }) {
  useLanguage();
  const [destination, setDestination] = useState("");
  const [method, setMethod] = useState(capture.method);
  const [rows, setRows] = useState(() => replayRows(capture));
  const [body, setBody] = useState(capture.body_base64);
  const [timeout, setTimeout] = useState(10000);
  const [verify, setVerify] = useState(true);
  const [redirects, setRedirects] = useState(false);
  const [credentials, setCredentials] = useState(false);
  const [busy, setBusy] = useState(false);
  const [response, setResponse] = useState<ApiResponse | null>(null);
  const [error, setError] = useState<ErrorCopy>("");
  const active = useRef<string | null>(null);
  const live = useRef(true);
  useEffect(() => { live.current = true; return () => {
    live.current = false;
    if (active.current) void api("/api/webhooks/replay/cancel", "POST", { replay_id: active.current }).catch(() => {});
  }; }, []);
  async function send() {
    if (active.current) return;
    const replay_id = crypto.randomUUID(); active.current = replay_id;
    setBusy(true); setError(""); setResponse(null);
    try {
      const result = await api<ApiResponse>(`/api/webhooks/${inbox}/replay`, "POST", { capture_id: capture.id, replay_id, destination, method, ...rows, body_base64: body, timeout_ms: timeout, verify_tls: verify, follow_redirects: redirects, include_credentials: credentials });
      if (live.current) setResponse(result);
    } catch (e) { if (live.current) setError(errorCopy(e)); }
    finally { active.current = null; if (live.current) setBusy(false); }
  }
  return <section className="webhook-replay">
    <Heading size="3">{t("编辑并重放")}</Heading>
    <Text size="2" color="gray">{t("默认移除认证、Cookie 和密钥字段。重放会向目标地址发送实际请求。")}</Text>
    <form onSubmit={e => { e.preventDefault(); void send(); }}>
      <fieldset disabled={busy} className="webhook-fields">
        <Flex gap="2" wrap="wrap" align="end">
          <Choice label={t("方法")} value={method} onChange={setMethod} options={["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"].map(value => ({ value, label: value }))} />
          <label className="webhook-destination"><Text as="div" size="2">{t("重放目标 URL")}</Text><TextField.Root type="url" required value={destination} onChange={e => setDestination(e.target.value)} placeholder="https://api.example.com/webhook" /></label>
          <label><Text as="div" size="2">{t("超时 (ms)")}</Text><TextField.Root type="number" min={100} max={120000} required value={timeout} onChange={e => setTimeout(Number(e.target.value))} /></label>
        </Flex>
        <Text as="div" size="2">{t("请求头")}</Text><PairEditor rows={rows.headers} onChange={headers => setRows({ ...rows, headers })} secrets />
        <Text as="div" size="2">{t("查询参数")}</Text><PairEditor rows={rows.query} onChange={query => setRows({ ...rows, query })} secrets />
        <label><Text as="div" size="2">{t("重放正文 (Base64)")}</Text><TextArea className="mono" value={body} onChange={e => setBody(e.target.value)} /></label>
        <Flex gap="3" wrap="wrap"><Text as="label" size="2"><Checkbox checked={verify} onCheckedChange={v => setVerify(v === true)} /> {t("验证 TLS 证书")}</Text><Text as="label" size="2"><Checkbox checked={redirects} onCheckedChange={v => setRedirects(v === true)} /> {t("跟随重定向")}</Text><Text as="label" size="2"><Checkbox checked={credentials} onCheckedChange={v => setCredentials(v === true)} /> {t("重放包含凭据")}</Text></Flex>
        <Button type="submit">{t("发送重放")}</Button>
      </fieldset>
      {busy && <Button type="button" color="gray" onClick={() => void api("/api/webhooks/replay/cancel", "POST", { replay_id: active.current }).catch(e => setError(errorCopy(e)))}>{t("取消重放")}</Button>}
    </form>
    {error && <Callout.Root color="red"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
    {response && <ResponsePane response={response} dark={dark} error="" busy={false} />}
  </section>;
}
