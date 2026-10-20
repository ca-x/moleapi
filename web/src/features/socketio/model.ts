import type { SocketIoConfig } from "../../shared/types";
export const socketIoConfig = (): SocketIoConfig => ({ kind: "socketio", namespace: "/", path: "/socket.io/", auth_source: "{}", listeners: ["message"], event: "message", arguments_source: "[]", attachments_base64: [], request_ack: false, ack_timeout_ms: 5000 });
