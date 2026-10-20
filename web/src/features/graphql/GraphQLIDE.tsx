import { useEffect, useRef, type MutableRefObject } from "react";
import { GraphiQL } from "graphiql";
import { useGraphiQL, useGraphiQLActions } from "@graphiql/react";
import "graphiql/style.css";
import "graphiql/setup-workers/vite";
import type { Storage, Fetcher } from "@graphiql/toolkit";
import type { GraphQLSchema } from "graphql";
import { t, useLanguage } from "../../shared/i18n";
import type { GraphQLConfig } from "../../shared/types";
const noEditorPersistence: Storage = {
  getItem: () => null,
  setItem: () => {},
  removeItem: () => {},
  clear: () => {},
  length: 0,
};
// Subscribe to Monaco changes synchronously; GraphiQL's onEdit callbacks are
// debounced and may run after the outgoing editor has been disposed.
function EditorBridge({ onQuery, onVariables, initialOperation, runRef }: {
  onQuery: (value: string) => void;
  onVariables: (value: string) => void;
  initialOperation?: string | null;
  runRef?: MutableRefObject<(() => void) | null>;
}) {
  const { language } = useLanguage();
  const queryEditor = useGraphiQL((store) => store.queryEditor);
  const responseEditor = useGraphiQL((store) => store.responseEditor);
  const variableEditor = useGraphiQL((store) => store.variableEditor);
  useEffect(() => {
    queryEditor?.updateOptions({ ariaLabel: t("GraphiQL:Operation Editor") });
    variableEditor?.updateOptions({ ariaLabel: t("GraphiQL:Variables") });
    responseEditor?.updateOptions({ ariaLabel: t("GraphiQL:Result Window") });
  }, [language, queryEditor, variableEditor, responseEditor]);
  const { run, setOperationName } = useGraphiQLActions();
  const savedOperation = useRef(initialOperation);
  const seededOperation = useRef(false);
  useEffect(() => {
    if (!queryEditor || seededOperation.current) return;
    seededOperation.current = true;
    if (savedOperation.current) setOperationName(savedOperation.current);
  }, [queryEditor, setOperationName]);
  const callbacks = useRef({ onQuery, onVariables });
  callbacks.current = { onQuery, onVariables };
  useEffect(() => {
    if (!runRef) return;
    runRef.current = run;
    return () => { if (runRef.current === run) runRef.current = null; };
  }, [runRef, run]);
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
export default function GraphQLIDE({ scope, config, schema, fetcher, dark, onChange, runRef }: {
  scope: string; config: GraphQLConfig; schema?: GraphQLSchema | null; fetcher: Fetcher; dark: boolean; onChange: (patch: Partial<GraphQLConfig>) => void; runRef?: MutableRefObject<(() => void) | null>;
}) {
  return <GraphiQL key={scope} fetcher={fetcher} schema={schema ?? null} storage={noEditorPersistence} maxHistoryLength={0} shouldPersistHeaders={false} isHeadersEditorEnabled={false} forcedTheme={dark ? "dark" : "light"} initialQuery={config.document} initialVariables={config.variables_source ?? JSON.stringify(config.variables, null, 2)} onEditOperationName={(operation_name) => onChange({ operation_name: operation_name || null })}>
    <EditorBridge initialOperation={config.operation_name} runRef={runRef} onQuery={(document) => onChange({ document })} onVariables={(variables_source) => {
      let parsed: unknown;
      try { parsed = JSON.parse(variables_source || "{}"); } catch { /* Retain invalid editor drafts. */ }
      onChange({ variables_source, ...(parsed && typeof parsed === "object" && !Array.isArray(parsed) ? { variables: parsed as Record<string, unknown> } : {}) });
    }} />
  </GraphiQL>;
}
