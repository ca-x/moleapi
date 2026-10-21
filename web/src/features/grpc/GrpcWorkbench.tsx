import { useRef, useState, useEffect } from "react";
import { useWorkbench } from "../workbench/context";
import { useGrpcSchema } from "./useGrpcSchema";
import GrpcClient from "./GrpcClient";
import ProtoSourceDialog from "./ProtoSourceDialog";
export default function GrpcWorkbench() {
  const state = useWorkbench(), source = useGrpcSchema();
  const latest = useRef(state); latest.current = state;
  const [open, setOpen] = useState(false);
  const scope = JSON.stringify([state.authenticated, state.accountId, state.draft?.id, state.request?.id, state.draft?.data.active_environment_id]);
  useEffect(() => { setOpen(false); }, [scope]);
  if (state.request?.protocol?.kind !== "grpc") return null;
  return <><GrpcClient key={scope} scope={scope} request={state.request} specifications={state.draft?.data.specifications ?? []} schema={source.schema} dark={state.dark} sourceError={source.error} sourceBusy={source.busy} onImport={() => setOpen(true)} onReflect={() => void source.reflect()} channel={state.protocolSession} isCurrent={source.guard()} onChange={(patch) => latest.current.updateRequest(patch)} /><ProtoSourceDialog open={open} onOpenChange={setOpen} source={source} /></>;
}
