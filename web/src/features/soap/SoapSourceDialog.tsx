import { api } from "../../shared/api";
import { useWorkbench } from "../workbench/context";
import type { useSoapSchema } from "./useSoapSchema";
import type { SoapSchemaResult } from "./types";
import SoapSourceEditor, { type SoapImportInput } from "./SoapSourceEditor";
export default function SoapSourceDialog({open,onOpenChange,source}:{open:boolean;onOpenChange:(open:boolean)=>void;source:ReturnType<typeof useSoapSchema>}) {
  const state=useWorkbench();
  async function importSource(input:SoapImportInput) {
    if(!state.draft) throw new Error("Workspace unavailable");
    const collection=state.draft.data.collections.find(c=>c.requests.some(r=>r.id===state.request?.id))?.id;
    const environment=state.draft.data.active_environment_id;
    const locals=state.localVariables.values(state.draft,collection,environment);
    return api<SoapSchemaResult>("url" in input?"/api/soap/import-url":"/api/soap/import","POST",{workspace_id:state.draft.id,environment_id:environment,...(locals.length?{locals}:{}),...input});
  }
  return <SoapSourceEditor open={open} onOpenChange={onOpenChange} source={source} dark={state.dark} importSource={importSource} />;
}
