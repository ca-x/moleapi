import { useState } from "react";
import { t, useLanguage } from "../../shared/i18n";
import type { RequestSpec } from "../../shared/types";
import GrpcClient, { type GrpcChannel } from "../grpc/GrpcClient";
import type { GrpcSchema } from "../grpc/types";
export default function SharedGrpcSessionControls(props: { request: RequestSpec; schema: GrpcSchema | null; channel: GrpcChannel; dark: boolean; isCurrent: () => boolean }) {
  useLanguage();
  const [message, setMessage] = useState(() => props.request.protocol?.kind === "grpc" ? props.request.protocol.message_source : "{}");
  const config = props.request.protocol;
  if (config?.kind !== "grpc") return null;
  const request = { ...props.request, protocol: { ...config, message_source: message } };
  return <section className="shared-grpc" aria-label={t("共享 gRPC 实时调用")}><GrpcClient scope={props.channel.session?.id ?? ""} request={request} specifications={[]} schema={props.schema} dark={props.dark} channel={props.channel} connectionSettings={false} isCurrent={props.isCurrent} onChange={(patch) => { if (props.isCurrent() && patch.protocol?.kind === "grpc") setMessage(patch.protocol.message_source); }} editorHeight="180px" /></section>;
}
