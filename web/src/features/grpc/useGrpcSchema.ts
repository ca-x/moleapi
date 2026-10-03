import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../../shared/api";
import { safeMessage } from "../../shared/model";
import type { Specification } from "../../shared/types";
import { useWorkbench } from "../workbench/context";
import type { GrpcSchema, GrpcSchemaResult, ReflectionResult } from "./types";
export function useGrpcSchema() {
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
    schema: GrpcSchema;
  } | null>(null);
  const [error, setError] = useState<{ scope: string; message: string } | null>(
    null,
  );
  const [pending, setPending] = useState<string | null>(null);
  const tickets = useRef(new Set<string>());
  const guard = useCallback(() => {
    const origin = scope.current;
    const generation = epoch.current;
    return () =>
      mounted.current &&
      scope.current === origin &&
      epoch.current === generation &&
      latest.current.authenticated;
  }, []);
  const attach = useCallback(
    (specification: Specification, schema: GrpcSchema) => {
      const origin = latest.current;
      const existing = origin.draft?.data.specifications?.find(
        (item) =>
          item.kind === specification.kind &&
          item.source === specification.source,
      );
      const spec = existing || specification;
      if (!existing)
        origin.updateData((data) => ({
          ...data,
          specifications: [...(data.specifications || []), spec],
        }));
      origin.updateRequest({ specification_id: spec.id });
      setResult({ scope: scope.current, specification: spec.id, schema });
      setError(null);
    },
    [],
  );
  const specificationId = state.request?.specification_id;
  const specification = state.draft?.data.specifications?.find(
    (item) => item.id === specificationId && item.kind === "protobuf",
  );
  const schema =
    result?.scope === identity && result.specification === specificationId
      ? result.schema
      : null;
  useEffect(() => {
    if (!specification || !state.draft || schema) return;
    const current = guard();
    let disposed = false;
    const origin = identity;
    void (async () => {
      try {
        if (latest.current.dirty && !(await latest.current.save(true)))
          throw new Error("请先解决工作区保存冲突");
        if (disposed || !current()) return;
        const loaded = await api<GrpcSchemaResult>("/api/grpc/schema", "POST", {
          workspace_id: latest.current.draft!.id,
          specification_id: specification.id,
        });
        if (!disposed && current()) {
          setResult({
            scope: origin,
            specification: specification.id,
            schema: loaded.schema,
          });
          setError(null);
        }
      } catch (caught) {
        if (!disposed && current())
          setError({ scope: origin, message: safeMessage(caught) });
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
    guard,
  ]);
  async function reflect() {
    const origin = latest.current;
    const draft = origin.draft;
    const request = origin.request;
    if (
      !draft ||
      request?.protocol?.kind !== "grpc" ||
      !origin.authenticated ||
      tickets.current.has(identity)
    )
      return;
    const current = guard();
    const ticket = identity;
    tickets.current.add(ticket);
    setPending(ticket);
    setError(null);
    const collection = draft.data.collections.find((item) =>
      item.requests.some((entry) => entry.id === request.id),
    )?.id;
    const environment = draft.data.active_environment_id;
    const locals = origin.localVariables.values(draft, collection, environment);
    try {
      if (origin.dirty && !(await origin.save(true)))
        throw new Error("请先解决工作区保存冲突");
      if (!current()) return;
      const reflected = await api<ReflectionResult>(
        "/api/grpc/reflect",
        "POST",
        {
          workspace_id: draft.id,
          request: structuredClone(request),
          environment_id: environment,
          ...(locals.length ? { locals } : {}),
        },
      );
      if (!current()) return;
      origin.localVariables.apply(
        draft,
        collection,
        environment,
        reflected.variable_updates || [],
      );
      if (reflected.error || !reflected.specification || !reflected.schema)
        throw new Error(
          reflected.error ||
            (reflected.status
              ? `${reflected.status.code} ${reflected.status.name}: ${reflected.status.message}`
              : "服务未返回可用定义"),
        );
      attach(reflected.specification, reflected.schema);
    } catch (caught) {
      if (current()) setError({ scope: ticket, message: safeMessage(caught) });
    } finally {
      tickets.current.delete(ticket);
      if (mounted.current)
        setPending((value) => (value === ticket ? null : value));
    }
  }
  return {
    schema,
    specification,
    busy: pending === identity,
    error: error?.scope === identity ? error.message : "",
    reflect,
    attach,
    guard,
  };
}
