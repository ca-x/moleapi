import type { ErrorCopy } from "../../shared/i18n/errors";
import { LocalizedError, errorCopy } from "../../shared/i18n/errors";
import { useLanguage, translateCopy } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { api } from "../../shared/api";
import type { Specification } from "../../shared/types";
import { useWorkbench } from "../workbench/context";
import type { SoapSchema, SoapSchemaResult } from "./types";
export function useSoapSchema() {
  useLanguage();
  const state = useWorkbench();
  const latest = useRef(state);
  latest.current = state;
  const identity = JSON.stringify([
    state.authenticated,
    state.accountId,
    state.draft?.id,
    state.request?.id,
    state.draft?.data.active_environment_id,
  ]);
  const scope = useRef(identity);
  const epoch = useRef(0);
  if (scope.current !== identity) {
    scope.current = identity;
    epoch.current++;
  }
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      epoch.current++;
    };
  }, []);
  const [result, setResult] = useState<{
    scope: string;
    specification: string;
    source: string;
    schema: SoapSchema;
  } | null>(null);
  const [error, setError] = useState<{ scope: string; message: ErrorCopy } | null>(
    null,
  );
  const [pending, setPending] = useState<string | null>(null);
  function guard() {
    const origin = scope.current,
      generation = epoch.current;
    return () =>
      mounted.current &&
      scope.current === origin &&
      epoch.current === generation &&
      latest.current.authenticated;
  }
  function attach(candidate: Specification, schema: SoapSchema) {
    const origin = latest.current;
    const existing = origin.draft?.data.specifications?.find(
      (item) =>
        item.kind === candidate.kind && item.source === candidate.source,
    );
    const specification = existing || candidate;
    if (!existing)
      origin.updateData((data) => ({
        ...data,
        specifications: [...(data.specifications || []), specification],
      }));
    origin.updateRequest({
      specification_id: specification.id,
      ...(origin.request?.protocol?.kind === "soap" ? {
        protocol: { ...origin.request.protocol, service: "", port: "", operation: "", action: "" },
      } : {}),
    });
    setResult({
      scope: scope.current,
      specification: specification.id,
      source: specification.source,
      schema,
    });
    setError(null);
  }
  const specification = state.draft?.data.specifications?.find(
    (item) =>
      item.id === state.request?.specification_id && item.kind === "wsdl",
  );
  const schema =
    result?.scope === identity &&
    result.specification === specification?.id &&
    result.source === specification.source
      ? result.schema
      : null;
  useEffect(() => {
    if (!specification || !state.draft || schema) return;
    const current = guard();
    let disposed = false;
    const origin = identity;
    setPending(origin);
    void (async () => {
      try {
        if (latest.current.dirty && !(await latest.current.save(true)))
          throw new LocalizedError("请先解决工作区保存冲突");
        if (disposed || !current()) return;
        const loaded = await api<SoapSchemaResult>("/api/soap/schema", "POST", {
          workspace_id: latest.current.draft!.id,
          specification_id: specification.id,
        });
        if (!disposed && current()) {
          setResult({
            scope: origin,
            specification: specification.id,
            source: specification.source,
            schema: loaded.schema,
          });
          setError(null);
        }
      } catch (caught) {
        if (!disposed && current())
          setError({ scope: origin, message: errorCopy(caught) });
      } finally {
        if (mounted.current)
          setPending((value) => (value === origin ? null : value));
      }
    })();
    return () => {
      disposed = true;
    };
  }, [
    specification?.id,
    specification?.source,
    state.draft?.id,
    identity,
    schema,
  ]);
  return {
    schema,
    specification,
    busy: pending === identity,
    error: error?.scope === identity ? translateCopy(error.message) : "",
    attach,
    guard,
  };
}
