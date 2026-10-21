import { useEffect, useRef, useState } from "react";
import { Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { RequestSpec } from "../../shared/types";
import GrpcClient from "../grpc/GrpcClient";
import ProtoSourceEditor from "../grpc/ProtoSourceEditor";
import SharedLiveSessionPanel from "./SharedLiveSessionPanel";
import { useSharedGrpcSchema, type SharedGrpcSourceContext } from "./useSharedGrpcSchema";
export default function SharedGrpcWorkbench(props: SharedGrpcSourceContext & { dark: boolean; onChange: (patch: Partial<RequestSpec>) => void }) {
  useLanguage();
  const source = useSharedGrpcSchema(props);
  const [open, setOpen] = useState(false);
  const latest = useRef(props); latest.current = props;
  const mounted = useRef(true); useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  function change(patch: Partial<RequestSpec>) { if (mounted.current && latest.current.scope === props.scope && latest.current.isCurrent() && !latest.current.saving && !latest.current.conflict) latest.current.onChange(patch); }
  const config = props.request.protocol;
  if (config?.kind !== "grpc") return null;
  const method = source.schema?.services.find((s) => s.name === config.service)?.methods.find((m) => m.name === config.method);
  return <section className="shared-grpc" aria-label={t("共享 gRPC 客户端")}>
    <Text as="p" size="1" color="gray">{t("定义与请求关联只在点击保存后写入共享工作区。")}</Text>
    <GrpcClient scope={props.scope} request={props.request} specifications={props.data.specifications ?? []} schema={source.schema} dark={props.dark} onChange={change} isCurrent={props.isCurrent} sourceError={source.error} sourceBusy={source.busy} sourceActionsDisabled={props.dirty || props.saving || props.conflict} disabled={props.saving || props.conflict} onImport={() => setOpen(true)} onReflect={() => void source.reflect()} editorHeight="180px" />
    <ProtoSourceEditor open={open} onOpenChange={setOpen} source={source} dark={props.dark} importSource={source.importSource} />
    <SharedLiveSessionPanel key={`${props.scope}/${props.dirty}`} base={props.base} request={props.request} revision={props.saved.revision} environmentId={props.saved.data.active_environment_id} dirty={props.dirty} saving={props.saving} conflict={props.conflict} dark={props.dark} isCurrent={props.isCurrent} onSaveResultDataset={async () => {}} grpcSchema={source.schema} connectDisabled={!method} />
  </section>;
}
