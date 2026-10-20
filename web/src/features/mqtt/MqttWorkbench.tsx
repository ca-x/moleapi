import { useRef } from "react";
import { useWorkbench } from "../workbench/context";
import MqttClient from "./MqttClient";
export default function MqttWorkbench() {
  const state = useWorkbench(), latest = useRef(state); latest.current = state;
  const scope = JSON.stringify([state.authenticated, state.accountId, state.draft?.id, state.request?.id, state.draft?.data.active_environment_id]);
  const current = useRef(scope); current.current = scope;
  if (state.request?.protocol?.kind !== "mqtt") return null;
  return <MqttClient key={scope} scope={scope} request={state.request} dark={state.dark} channel={state.protocolSession} isCurrent={() => current.current === scope} onChange={(patch) => latest.current.updateRequest(patch)} />;
}
