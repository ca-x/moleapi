import { useLanguage } from "../../shared/i18n";
import SessionEventPane from "../protocols/SessionEventPane";
import type { ComponentProps } from "react";
export default function GrpcEvents(
  props: ComponentProps<typeof SessionEventPane>,
) {
  useLanguage();
  return <SessionEventPane {...props} protocolLabel="gRPC" />;
}
