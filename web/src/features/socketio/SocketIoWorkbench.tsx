import { useRef } from "react";
import { useWorkbench } from "../workbench/context";
import SocketIoClient from "./SocketIoClient";

export default function SocketIoWorkbench() {
  const state = useWorkbench();
  const latest = useRef(state); latest.current = state;
  const config = state.request?.protocol;
  if (config?.kind !== "socketio") return null;
  const scope = JSON.stringify([state.authenticated, state.accountId, state.draft?.id, state.request?.id, state.draft?.data.active_environment_id]);
  return <SocketIoClient key={scope} scope={scope} config={config} dark={state.dark} isCurrent={() => true} channel={state.protocolSession} onChange={(patch) => {
    const current = latest.current.request?.protocol;
    if (current?.kind === "socketio") latest.current.updateRequest({ protocol: { ...current, ...patch } });
  }} />;
}
