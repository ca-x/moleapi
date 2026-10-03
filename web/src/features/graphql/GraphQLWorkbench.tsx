import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Badge, Button, Callout, Flex, Text, Dialog } from "@radix-ui/themes";
import { RefreshCw, FileText } from "lucide-react";
import { GraphiQL } from "graphiql";
import { useGraphiQL, useGraphiQLActions } from "@graphiql/react";
import "graphiql/style.css";
import "graphiql/setup-workers/vite";
import { buildSchema } from "graphql";
import type { Storage } from "@graphiql/toolkit";
import { api } from "../../shared/api";
import { safeMessage } from "../../shared/model";
import type {
  ApiResponse,
  GraphQLConfig,
  Specification,
} from "../../shared/types";
import { useWorkbench } from "../workbench/context";
import { ResponsePane } from "../requests/ResponsePane";
import { createRustGraphQLFetcher } from "./fetcher";
const noEditorPersistence: Storage = {
  getItem: () => null,
  setItem: () => {},
  removeItem: () => {},
  clear: () => {},
  length: 0,
};
// Subscribe to Monaco changes synchronously; GraphiQL's onEdit callbacks are
// debounced and may run after the outgoing editor has been disposed.
function EditorBridge({ onQuery, onVariables, initialOperation }: {
  onQuery: (value: string) => void;
  onVariables: (value: string) => void;
  initialOperation?: string | null;
}) {
  const queryEditor = useGraphiQL((store) => store.queryEditor);
  const variableEditor = useGraphiQL((store) => store.variableEditor);
  const { run, setOperationName } = useGraphiQLActions();
  const savedOperation = useRef(initialOperation);
  const seededOperation = useRef(false);
  useEffect(() => {
    if (!queryEditor || seededOperation.current) return;
    seededOperation.current = true;
    if (savedOperation.current) setOperationName(savedOperation.current);
  }, [queryEditor, setOperationName]);
  const { graphqlRun } = useWorkbench();
  const callbacks = useRef({ onQuery, onVariables });
  callbacks.current = { onQuery, onVariables };
  useEffect(() => {
    graphqlRun.current = run;
    return () => { if (graphqlRun.current === run) graphqlRun.current = null; };
  }, [graphqlRun, run]);
  useEffect(() => {
    const listener = queryEditor?.onDidChangeModelContent(() =>
      callbacks.current.onQuery(queryEditor.getValue()));
    return () => listener?.dispose();
  }, [queryEditor]);
  useEffect(() => {
    const listener = variableEditor?.onDidChangeModelContent(() =>
      callbacks.current.onVariables(variableEditor.getValue()));
    return () => listener?.dispose();
  }, [variableEditor]);
  return null;
}
export default function GraphQLWorkbench() {
  const state = useWorkbench();
  const latest = useRef(state);
  latest.current = state;
  const mounted = useRef(true);
  const sessions = useMemo(() => new Set<string>(), []);
  const scope = JSON.stringify([
    state.authenticated,
    state.accountId,
    state.draft?.id,
    state.request?.id,
    state.draft?.data.active_environment_id,
  ]);
  const scopeRef = useRef(scope);
  const generation = useRef(0);
  if (scopeRef.current !== scope) {
    scopeRef.current = scope;
    generation.current++;
  }
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      generation.current++;
      for (const id of sessions)
        void api(
          `/api/sessions/${encodeURIComponent(id)}/close`,
          "POST",
          {},
        ).catch(() => {});
      sessions.clear();
    };
  }, [sessions, scope]);
  const [last, setLast] = useState<{
    scope: string;
    response: ApiResponse;
  } | null>(null);
  const [schema, setSchema] = useState<{ scope: string; sdl: string } | null>(
    null,
  );
  const [schemaError, setSchemaError] = useState<{
    scope: string;
    message: string;
  } | null>(null);
  const [loadingScope, setLoadingScope] = useState<string | null>(null);
  const loading = loadingScope === scope;
  const getContext = useCallback(async () => {
    const origin = latest.current;
    const draft = origin.draft;
    const request = origin.request;
    if (
      !origin.authenticated ||
      !draft ||
      request?.protocol?.kind !== "graphql"
    )
      throw new Error("没有可执行的 GraphQL 请求");
    const identity = scopeRef.current;
    const epoch = generation.current;
    const current = () =>
      mounted.current &&
      scopeRef.current === identity &&
      generation.current === epoch &&
      latest.current.authenticated;
    const snapshot = structuredClone(request);
    const collection = draft.data.collections.find((item) =>
      item.requests.some((entry) => entry.id === snapshot.id),
    )?.id;
    const environment = draft.data.active_environment_id;
    const locals = origin.localVariables.values(draft, collection, environment);
    if (origin.dirty && !(await origin.save(true)))
      throw new Error("请先解决工作区保存冲突");
    if (!current()) throw new Error("GraphQL 请求已取消");
    return {
      workspace_id: draft.id,
      request: snapshot,
      environment_id: environment,
      locals,
      current,
      sessions,
      apply: (updates: Parameters<typeof origin.localVariables.apply>[3]) => {
        if (current())
          origin.localVariables.apply(draft, collection, environment, updates);
      },
      response: (response: ApiResponse) => {
        if (current()) setLast({ scope: identity, response });
      },
    };
  }, [sessions]);
  const fetcher = useMemo(
    () => createRustGraphQLFetcher(getContext),
    [getContext],
  );
  const gql =
    state.request?.protocol?.kind === "graphql" ? state.request.protocol : null;
  const activeSchema = schema?.scope === scope ? schema.sdl : undefined;
  const builtSchema = useMemo(() => {
    try {
      return activeSchema ? buildSchema(activeSchema) : undefined;
    } catch {
      return null;
    }
  }, [activeSchema]);
  function update(patch: Partial<GraphQLConfig>) {
    const current = latest.current.request;
    if (mounted.current && scopeRef.current === scope && current?.protocol?.kind === "graphql")
      latest.current.updateRequest({
        protocol: { ...current.protocol, ...patch },
      });
  }
  const specificationId = state.request?.specification_id;
  useEffect(() => {
    if (!specificationId || !state.draft || activeSchema) return;
    const specification = state.draft.data.specifications?.find(
      (item) => item.id === specificationId);
    if (!specification?.kind.startsWith("graphql-")) return;
    let cancelled = false;
    const origin = scope;
    const epoch = generation.current;
    void (async () => {
      try {
        // Newly imported sources must be saved before the Rust schema endpoint
        // can read them, including in the independent offline desktop client.
        if (latest.current.dirty && !(await latest.current.save(true)))
          throw new Error("请先解决工作区保存冲突");
        if (cancelled || scopeRef.current !== origin || generation.current !== epoch) return;
        const result = await api<{sdl: string}>("/api/graphql/schema", "POST", {
          workspace_id: latest.current.draft!.id,
          specification_id: specificationId,
        });
        if (!cancelled && scopeRef.current === origin && generation.current === epoch)
          setSchema({ scope: origin, sdl: result.sdl });
      } catch (error) {
        if (!cancelled && scopeRef.current === origin && generation.current === epoch)
          setSchemaError({ scope: origin, message: safeMessage(error) });
      }
    })();
    return () => { cancelled = true; };
  }, [specificationId, scope, activeSchema, state.draft?.id]);
  async function introspect() {
    if (loading) return;
    const origin = scopeRef.current;
    const epoch = generation.current;
    const current = () =>
      mounted.current &&
      scopeRef.current === origin &&
      generation.current === epoch;
    setLoadingScope(origin);
    setSchemaError(null);
    try {
      const context = await getContext();
      const result = await api<{
        response: ApiResponse;
        specification?: Specification;
        sdl?: string;
        error?: string;
      }>("/api/graphql/introspect", "POST", {
        workspace_id: context.workspace_id,
        request: context.request,
        environment_id: context.environment_id,
        ...(context.locals.length ? { locals: context.locals } : {}),
      });
      if (!context.current()) return;
      context.apply(result.response.variable_updates || []);
      context.response(result.response);
      if (result.error || !result.sdl || !result.specification)
        throw new Error(result.error || "服务未返回可用 schema");
      const candidate = result.specification;
      const existing = latest.current.draft?.data.specifications?.find(
        (item) =>
          item.kind === candidate.kind && item.source === candidate.source,
      );
      const spec = existing || candidate;
      if (!existing)
        latest.current.updateData((data) => ({
          ...data,
          specifications: [...(data.specifications || []), spec],
        }));
      latest.current.updateRequest({ specification_id: spec.id });
      setSchema({ scope: scopeRef.current, sdl: result.sdl });
    } catch (error) {
      if (current())
        setSchemaError({
          scope: origin,
          message: safeMessage(error),
        });
    } finally {
      if (mounted.current)
        setLoadingScope((value) => (value === origin ? null : value));
    }
  }
  if (!gql) return null;
  const response = last?.scope === scope ? last.response : null;
  return (
    <section className="graphql-workbench" aria-label="GraphQL 客户端">
      <Flex
        className="graphql-toolbar"
        align="center"
        justify="between"
        gap="3"
      >
        <Flex gap="3" align="center">
          <Text size="2" weight="medium">
            GraphQL
          </Text>
          {response && (
            <>
              <Badge color={response.status < 400 ? "green" : "red"}>
                HTTP {response.status}
              </Badge>
              <Text size="1" className="mono" color="gray">
                {response.elapsed_ms} ms
              </Text>
            </>
          )}
        </Flex>
        <Button
          size="1"
          variant="soft"
          color="gray"
          loading={loading}
          onClick={() => void introspect()}
        >
          <RefreshCw size={14} />
          读取并保存 Schema
        </Button>
      </Flex>
      {schemaError?.scope === scope && (
        <Callout.Root color="red" role="alert">
          <Callout.Text>{schemaError.message}</Callout.Text>
        </Callout.Root>
      )}
      {activeSchema && builtSchema === null && (
        <Callout.Root color="red">
          <Callout.Text>Schema 无法加载，请检查规范或重新内省。</Callout.Text>
        </Callout.Root>
      )}
      {response && (
        <Dialog.Root>
          <Dialog.Trigger>
            <Button size="1" variant="ghost" color="gray">
              <FileText size={14} />
              查看 HTTP / 脚本结果
            </Button>
          </Dialog.Trigger>
          <Dialog.Content maxWidth="850px">
            <Dialog.Title>本次 GraphQL 请求的 HTTP / 脚本结果</Dialog.Title>
            <ResponsePane
              response={response}
              error=""
              dark={state.dark}
              busy={false}
            />
            <Flex justify="end" mt="3">
              <Dialog.Close>
                <Button variant="soft" color="gray">
                  关闭
                </Button>
              </Dialog.Close>
            </Flex>
          </Dialog.Content>
        </Dialog.Root>
      )}
      <div className="graphql-ide">
        <GraphiQL
          key={scope}
          fetcher={fetcher}
          schema={builtSchema ?? null}
          storage={noEditorPersistence}
          maxHistoryLength={0}
          shouldPersistHeaders={false}
          isHeadersEditorEnabled={false}
          forcedTheme={state.dark ? "dark" : "light"}
          initialQuery={gql.document}
          initialVariables={
            gql.variables_source ?? JSON.stringify(gql.variables, null, 2)
          }
          onEditOperationName={(operation_name) =>
            update({ operation_name: operation_name || null })
          }
        >
          <EditorBridge
            initialOperation={gql.operation_name}
            onQuery={(document) => update({ document })}
            onVariables={(variables_source) => {
              let parsed: unknown;
              try { parsed = JSON.parse(variables_source || "{}"); } catch { /* Preserve invalid drafts. */ }
              update({ variables_source,
                ...(parsed && typeof parsed === "object" && !Array.isArray(parsed)
                  ? { variables: parsed as Record<string, unknown> } : {}),
              });
            }}
          />
        </GraphiQL>
      </div>
    </section>
  );
}
