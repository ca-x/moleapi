import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button, Callout, Dialog, Flex, Text, TextField } from "@radix-ui/themes";
import { buildSchema } from "graphql";
import { api, token } from "../../shared/api";
import { t, useLanguage } from "../../shared/i18n";
import { errorCopy, LocalizedError, type ErrorCopy } from "../../shared/i18n/errors";
import { translateCopy } from "../../shared/i18n";
import { Choice, Editor } from "../../shared/ui";
import type { ApiResponse, GraphQLConfig, RequestSpec, Specification, Workspace, WorkspaceData } from "../../shared/types";
import GraphQLIDE from "../graphql/GraphQLIDE";
import { createRustGraphQLFetcher, type GraphQLContext } from "../graphql/fetcher";
import { ResponsePane } from "../requests/ResponsePane";
import type { ProtocolSession } from "../protocols/types";
import { savedSubscriptionMatches } from "./sharedGraphQL";
export default function SharedGraphQLWorkbench(props: {
  scope: string; base: string; request: RequestSpec; saved: Workspace; data: WorkspaceData; collectionId: string; dirty: boolean; saving: boolean; conflict: boolean; dark: boolean;
  isCurrent: () => boolean; onChange: (patch: Partial<RequestSpec>) => void; onAttach: (spec: Specification) => string | undefined;
}) {
  useLanguage();
  const latest = useRef(props); latest.current = props;
  const mounted = useRef(true), sessions = useRef(new Set<string>()), credential = useRef(token());
  const identity = JSON.stringify([props.scope, props.request]);
  const currentIdentity = useRef(identity); currentIdentity.current = identity;
  const [last, setLast] = useState<{ identity: string; response: ApiResponse } | null>(null), [schema, setSchema] = useState<string | null>(null), [problem, setProblem] = useState<ErrorCopy>(""), [busy, setBusy] = useState(false);
  const [params, setParams] = useState<string | null>(null);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; for (const id of sessions.current) void fetch(`${props.base}/sessions/${encodeURIComponent(id)}/close`, { method: "POST", headers: { Authorization: `Bearer ${credential.current}` } }).catch(() => {}); sessions.current.clear(); }; }, []);
  const getContext = useCallback(async (): Promise<GraphQLContext> => {
    const origin = latest.current, originIdentity = currentIdentity.current;
    const current = () => mounted.current && currentIdentity.current === originIdentity && latest.current.isCurrent() && !latest.current.conflict;
    if (!current() || origin.request.protocol?.kind !== "graphql") throw new LocalizedError("GraphQL 请求已取消");
    if (origin.request.specification_id && !origin.saved.data.specifications?.some((spec) => spec.id === origin.request.specification_id)) throw new LocalizedError("先保存工作区，再使用关联 Schema。");
    return { workspace_id: origin.saved.id, request: structuredClone(origin.request), environment_id: origin.saved.data.active_environment_id, locals: [], sessions: sessions.current, current,
      apply: () => {}, response: (response) => { if (current()) setLast({ identity: originIdentity, response }); },
      transport: {
        execute: async (request) => { if (!current() || latest.current.saving) throw new LocalizedError("GraphQL 请求已取消"); return api<ApiResponse>(`${origin.base}/execute-draft`, "POST", { request, collection_id: origin.collectionId, expected_workspace_revision: origin.saved.revision }); },
        createSession: async (request) => { if (!current() || latest.current.dirty || latest.current.saving || !savedSubscriptionMatches(origin.request, request)) throw new LocalizedError("先保存 GraphQL 请求及变量，再订阅。"); return api<ProtocolSession>(`${origin.base}/sessions`, "POST", { request_id: origin.request.id, expected_revision: origin.saved.revision, environment_id: origin.saved.data.active_environment_id }); },
        sessionPath: (id) => `${origin.base}/sessions/${encodeURIComponent(id)}`,
      },
    };
  }, []);
  const fetcher = useMemo(() => createRustGraphQLFetcher(getContext), [getContext]);
  function requestChange(patch: Partial<RequestSpec>) {
    if (mounted.current && latest.current.isCurrent() && !latest.current.conflict) latest.current.onChange(patch);
  }
  function change(patch: Partial<GraphQLConfig>) { const config = latest.current.request.protocol; if (config?.kind === "graphql") requestChange({ protocol: { ...config, ...patch } }); }
  const specificationId = props.request.specification_id;
  useEffect(() => {
    if (props.dirty || props.conflict) return;
    setSchema(null); setProblem("");
    if (!specificationId) return;
    let cancelled = false;
    const origin = props.scope, originalIdentity = currentIdentity.current;
    void api<{ sdl: string }>(`${props.base}/graphql/schema`, "POST", { request_id: props.request.id, specification_id: specificationId, expected_revision: props.saved.revision }).then((result) => { if (!cancelled && mounted.current && latest.current.scope === origin && currentIdentity.current === originalIdentity && !latest.current.dirty && !latest.current.conflict && latest.current.isCurrent()) setSchema(result.sdl); }).catch((cause) => { if (!cancelled && mounted.current && latest.current.scope === origin && currentIdentity.current === originalIdentity && latest.current.isCurrent()) setProblem(errorCopy(cause)); });
    return () => { cancelled = true; };
  }, [specificationId, props.scope, props.saved.revision]);
  async function introspect() {
    if (busy || props.dirty || props.saving || props.conflict || !props.isCurrent()) return;
    const origin = identity;
    const current = () => mounted.current && currentIdentity.current === origin && latest.current.isCurrent() && !latest.current.dirty && !latest.current.conflict;
    setBusy(true); setProblem("");
    try {
      const result = await api<{ response: ApiResponse; specification?: Specification; sdl?: string; error?: string }>(`${props.base}/graphql/introspect`, "POST", { request_id: props.request.id, expected_revision: props.saved.revision });
      if (!current()) return;
      setLast({ identity: origin, response: result.response });
      if (result.error) throw new Error(result.error);
      if (!result.specification || !result.sdl) throw new LocalizedError("服务未返回可用 schema");
      const attached = props.onAttach(result.specification);
      if (attached) { setSchema(result.sdl); setLast({ identity: JSON.stringify([props.scope, { ...props.request, specification_id: attached }]), response: result.response }); }
    } catch (cause) { if (current()) setProblem(errorCopy(cause)); }
    finally { if (mounted.current) setBusy(false); }
  }
  const builtSchema = useMemo(() => { try { return schema ? buildSchema(schema) : null; } catch { return null; } }, [schema]);
  const config = props.request.protocol?.kind === "graphql" ? props.request.protocol : null;
  const schemas = (props.data.specifications ?? []).filter((spec) => spec.kind.startsWith("graphql-"));
  const missing = !!specificationId && !schemas.some((spec) => spec.id === specificationId);
  let parsedParams: Record<string, unknown> | null = null;
  try { const parsed: unknown = JSON.parse(params ?? "{}"); if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) parsedParams = parsed as Record<string, unknown>; } catch { /* Retain unapplied invalid text. */ }
  if (!config) return null;
  return <section className="shared-graphql" aria-label={t("共享 GraphQL 客户端")}><Flex direction="column" gap="3">
    <Flex gap="2" wrap="wrap"><Button loading={busy} disabled={busy || props.dirty || props.saving || props.conflict} onClick={() => void introspect()}>{t("读取 Schema")}</Button><Button variant="outline" onClick={() => setParams(JSON.stringify(config.connection_params, null, 2))}>{t("编辑订阅连接参数")}</Button></Flex>
    <Text size="1" color="gray">{t("查询与 mutation 可执行当前编辑内容；订阅与 Schema 内省使用已保存请求。读取 Schema 后，请保存共享工作区。")}</Text>
    <Choice label={t("GraphQL Schema")} value={specificationId ? `spec:${specificationId}` : "none"} options={[{ value: "none", label: t("不关联规范") }, ...schemas.map((spec) => ({ value: `spec:${spec.id}`, label: spec.name })), ...(missing ? [{ value: `spec:${specificationId}`, label: t("已保存的规范关联不可用") }] : [])]} onChange={(value) => { setSchema(null); requestChange({ specification_id: value === "none" ? null : value.slice(5) }); }} />
    <Text as="label" size="2">{t("GraphQL 订阅地址")}<TextField.Root value={config.subscription_url ?? ""} placeholder="ws:// / wss://" onChange={(event) => change({ subscription_url: event.target.value || null })} /></Text>
    {problem && <Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(problem)}</Callout.Text></Callout.Root>}
    <div className="graphql-ide"><GraphQLIDE scope={props.scope} config={config} schema={builtSchema} fetcher={fetcher} dark={props.dark} onChange={change} /></div>
    {last?.identity === identity && <Dialog.Root><Dialog.Trigger><Button variant="outline">{t("查看 HTTP / 脚本结果")}</Button></Dialog.Trigger><Dialog.Content maxWidth="min(850px, calc(100vw - 24px))"><Dialog.Title>{t("本次 GraphQL 请求的 HTTP / 脚本结果")}</Dialog.Title><ResponsePane response={last.response} error="" dark={props.dark} busy={false} /><Dialog.Close><Button>{t("关闭")}</Button></Dialog.Close></Dialog.Content></Dialog.Root>}
  </Flex><Dialog.Root open={params !== null} onOpenChange={(open) => { if (!open) setParams(null); }}><Dialog.Content maxWidth="min(560px, calc(100vw - 24px))"><Dialog.Title>{t("编辑订阅连接参数")}</Dialog.Title><Dialog.Description>{t("连接参数必须是 JSON 对象；应用后请保存请求再订阅。")}</Dialog.Description><Editor label={t("订阅连接参数 JSON")} value={params ?? ""} onChange={setParams} dark={props.dark} jsonMode height="220px" /><Flex gap="2" justify="end" mt="3"><Button variant="soft" onClick={() => setParams(null)}>{t("取消")}</Button><Button disabled={!parsedParams || new TextEncoder().encode(JSON.stringify(parsedParams)).length > 256 * 1024} onClick={() => { if (parsedParams) { change({ connection_params: parsedParams }); setParams(null); } }}>{t("应用连接参数")}</Button></Flex></Dialog.Content></Dialog.Root></section>;
}
