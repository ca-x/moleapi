import { useCallback, useEffect, useRef, useState } from "react";
import { memberApi as api } from "../../shared/memberApi";
import { useLanguage, translateCopy } from "../../shared/i18n";
import { errorCopy, LocalizedError, type ErrorCopy } from "../../shared/i18n/errors";
import type { RequestSpec, Specification, Workspace, WorkspaceData } from "../../shared/types";
import type { GrpcSchema, GrpcSchemaResult, ReflectionResult } from "../grpc/types";
export interface SharedGrpcSourceContext { scope: string; base: string; request: RequestSpec; saved: Workspace; data: WorkspaceData; dirty: boolean; saving: boolean; conflict: boolean; isCurrent: () => boolean; onAttach: (specification: Specification) => string | undefined; }
export function useSharedGrpcSchema(props: SharedGrpcSourceContext) {
  useLanguage();
  const latest = useRef(props); latest.current = props;
  const mounted = useRef(true);
  const identity = JSON.stringify([props.scope, props.request, props.saved.revision]);
  const currentIdentity = useRef(identity); currentIdentity.current = identity;
  const pending = useRef(false);
  const [busy, setBusy] = useState(false), [problem, setProblem] = useState<ErrorCopy>("");
  const [result, setResult] = useState<{ scope: string; id: string; schema: GrpcSchema } | null>(null);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const guard = useCallback(() => {
    const origin = identity;
    return () => mounted.current && currentIdentity.current === origin && latest.current.isCurrent() && !latest.current.dirty && !latest.current.saving && !latest.current.conflict;
  }, [identity]);
  const specification = props.data.specifications?.find((s) => s.id === props.request.specification_id && s.kind === "protobuf");
  const schema = result?.scope === props.scope && result.id === props.request.specification_id ? result.schema : null;
  function attach(specification: Specification, schema: GrpcSchema) {
    if (!guard()()) return;
    const origin = latest.current, id = origin.onAttach(specification);
    if (id) { setResult({ scope: origin.scope, id, schema }); setProblem(""); }
  }
  const specificationId = props.request.specification_id;
  useEffect(() => {
    if (props.dirty || props.saving || props.conflict || !specificationId || schema) return;
    const current = guard(); let disposed = false;
    const origin = props.scope;
    void api<GrpcSchemaResult>(`${props.base}/grpc/schema`, "POST", { request_id: props.request.id, specification_id: specificationId, expected_revision: props.saved.revision }).then((loaded) => { if (!disposed && current()) { setResult({ scope: origin, id: specificationId, schema: loaded.schema }); setProblem(""); } }).catch((error) => { if (!disposed && current()) setProblem(errorCopy(error)); });
    return () => { disposed = true; };
  }, [props.scope, props.saved.revision, specificationId, guard, props.dirty, props.saving, props.conflict, schema]);
  async function importSource(name: string, source: string) {
    const current = guard(), origin = latest.current;
    if (!current()) throw new LocalizedError("先保存工作区，再导入或读取 gRPC 定义。");
    const candidate = await api<GrpcSchemaResult>(`${origin.base}/grpc/import`, "POST", { request_id: origin.request.id, expected_revision: origin.saved.revision, name, source });
    if (!current()) throw new LocalizedError("gRPC 操作已取消");
    return candidate;
  }
  async function reflect() {
    const current = guard(), origin = latest.current;
    if (pending.current || !current()) return;
    pending.current = true; setBusy(true); setProblem("");
    try {
      const value = await api<ReflectionResult>(`${origin.base}/grpc/reflect`, "POST", { request_id: origin.request.id, expected_revision: origin.saved.revision });
      if (!current()) return;
      if (value.error) throw new Error(value.error);
      if (!value.schema || !value.specification) { if (value.status) throw new Error(`${value.status.code} ${value.status.name}: ${value.status.message}`); throw new LocalizedError("服务未返回可用定义"); }
      attach(value.specification, value.schema);
    } catch (error) { if (current()) setProblem(errorCopy(error)); }
    finally { pending.current = false; if (mounted.current) setBusy(false); }
  }
  return { schema, specification, busy, error: translateCopy(problem), guard, attach, importSource, reflect };
}
