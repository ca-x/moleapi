import { useEffect, useRef, useState } from "react";
import { Button, Flex, Heading, Text, TextArea, TextField } from "@radix-ui/themes";
import { memberApi as api, memberCredential as token } from "../../shared/memberApi";
import { t, translateCopy, useLanguage } from "../../shared/i18n";
import { errorCopy, type ErrorCopy } from "../../shared/i18n/errors";
import type { ApiResponse, Pair, RequestSpec, Workspace } from "../../shared/types";
import { Choice, Field, PairEditor } from "../../shared/ui";
import { mockRouteInfo } from "../requests/mockRouteInfo";
import { ResponsePane } from "../requests/ResponsePane";

type Envelope = ApiResponse & { matched: { request_id: string; example_id: string; outcome: string } | null; outcome: string };
type Input = { method: string; path: string; query: Pair[]; headers: Pair[]; body: string };
type Props = { scope: string; base: string; request: RequestSpec; saved: Workspace; dark: boolean; disabled: boolean; isCurrent: () => boolean; onExecuted: () => void };
export default function SharedMockRouteDebugger(props: Props) {
  useLanguage();
  const [input, setInput] = useState<Input>(() => ({ method: props.request.method, path: mockRouteInfo(props.request).path || "/", query: [], headers: [], body: "" }));
  const latest = useRef(props); latest.current = props;
  const identity = JSON.stringify([props.scope, props.base, props.request, props.saved.revision, props.disabled, input]);
  const currentIdentity = useRef(identity), epoch = useRef(0);
  if (currentIdentity.current !== identity) { currentIdentity.current = identity; epoch.current++; }
  const generation = epoch.current;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const [active, setActive] = useState<number | null>(null);
  const [result, setResult] = useState<{ generation: number; value: Envelope } | null>(null);
  const [error, setError] = useState<{ generation: number; value: ErrorCopy } | null>(null);
  const pending = useRef<number | null>(null);
  const credential = token();
  function admitted() { return mounted.current && currentIdentity.current === identity && epoch.current === generation && token() === credential && !latest.current.disabled && latest.current.isCurrent(); }
  function change(patch: Partial<Input>) { if (currentIdentity.current !== identity) return; epoch.current++; setInput(value => ({ ...value, ...patch })); setResult(null); setError(null); }
  const busy = active === generation;
  async function run() {
    if (!admitted() || busy || pending.current === generation) return;
    pending.current = generation; setActive(generation); setResult(null); setError(null);
    const pairs = (rows: Pair[]) => rows.map(({ key, value, enabled }) => ({ key, value, enabled }));
    try {
      const response = await api<Envelope>(`${props.base}/mock/route-preview`, "POST", { expected_revision: props.saved.revision, method: input.method, path: input.path, query: pairs(input.query), headers: pairs(input.headers), body: input.body });
      if (admitted()) { setResult({ generation, value: response }); latest.current.onExecuted(); }
    } catch (failure) { if (admitted()) setError({ generation, value: errorCopy(failure) }); }
    finally { if (pending.current === generation) pending.current = null; if (admitted()) setActive(null); }
  }
  const response = result?.generation === generation && admitted() ? result.value : null;
  const selected = response?.matched;
  const request = selected ? props.saved.data.collections.flatMap(c => c.requests).find(r => r.id === selected.request_id) : null;
  const example = request?.examples.find(e => e.id === selected?.example_id);
  return <Flex direction="column" gap="3" asChild><section aria-label={t("Mock 路由调试")}>
    <Heading size="3">{t("Mock 路由调试")}</Heading>
    <Text size="1" color="gray">{t("临时请求只用于匹配已保存的 Mock；不会保存，也不会发送到外部服务。")}</Text>
    <Flex gap="2" wrap="wrap" className="shared-request-line">
      <Choice label={t("测试方法")} value={input.method} options={["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"].map(value => ({ value, label: value }))} onChange={method => change({ method })} />
      <Field label={t("测试路径")}><TextField.Root value={input.path} placeholder="/orders/7" maxLength={4096} onChange={event => change({ path: event.target.value })} /></Field>
    </Flex>
    <Text size="1" color="gray">{t("将 {name} 替换为具体路径值；查询参数在下方填写，可重复同名参数。")}</Text>
    <div><Text as="p" size="2" weight="medium">{t("测试查询参数")}</Text><PairEditor rows={input.query} onChange={query => change({ query })} keyLabel={t("参数名")} valueLabel={t("值")} /></div>
    <div><Text as="p" size="2" weight="medium">{t("测试请求头")}</Text><PairEditor rows={input.headers} onChange={headers => change({ headers })} keyLabel="Header" valueLabel={t("值")} /></div>
    <Text size="1" color="gray">{t("不支持凭据类参数、Authorization、Cookie 或连接控制请求头。正文上限 1 MiB，响应上限 5 MiB。")}</Text>
    <Field label={t("测试请求正文")}><TextArea value={input.body} onChange={event => change({ body: event.target.value })} /></Field>
    <div><Button loading={busy} disabled={props.disabled || busy} onClick={() => void run()}>{t("测试 Mock 路由")}</Button></div>
    {props.disabled && <Text size="1" color="gray">{t("先保存修改并解决版本冲突，再测试 Mock 路由。")}</Text>}
    {response && <Text role="status" size="2">{example && request ? t("实际匹配：{{request}} / {{example}}", { request: request.name, example: example.name }) : t("没有可识别的匹配示例")} · {t("结果")}: {response.outcome === "served" ? t("已响应") : response.outcome === "unmatched" ? t("未匹配") : response.outcome === "simulated_failure" ? t("模拟故障") : response.outcome === "rate_limited" ? t("已限流") : t("失败")}</Text>}
    <ResponsePane response={response} error={error?.generation === generation && admitted() ? translateCopy(error.value) : ""} dark={props.dark} busy={busy} />
  </section></Flex>;
}
