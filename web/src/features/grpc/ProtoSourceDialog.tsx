import { api } from "../../shared/api";
import { LocalizedError } from "../../shared/i18n/errors";
import { useWorkbench } from "../workbench/context";
import ProtoSourceEditor, { type ProtoSourceController } from "./ProtoSourceEditor";
import type { GrpcSchemaResult } from "./types";
export default function ProtoSourceDialog({ open, onOpenChange, source }: { open: boolean; onOpenChange: (open: boolean) => void; source: ProtoSourceController }) {
  const state = useWorkbench();
  return <ProtoSourceEditor open={open} onOpenChange={onOpenChange} source={source} dark={state.dark} importSource={(name, source) => {
    if (!state.draft) throw new LocalizedError("请先解决工作区保存冲突");
    return api<GrpcSchemaResult>("/api/grpc/import", "POST", { workspace_id: state.draft.id, name, source });
  }} />;
}
