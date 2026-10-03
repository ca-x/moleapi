import SessionEventPane from "../protocols/SessionEventPane";
import type { ComponentProps } from "react";
export default function GrpcEvents(
  props: ComponentProps<typeof SessionEventPane>,
) {
  return <SessionEventPane {...props} protocolLabel="gRPC" />;
}
