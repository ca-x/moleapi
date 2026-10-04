export interface TcpMessage {encoding:"text"|"hex"|"base64";payload_source:string;secret:boolean}
export interface TcpConfig {kind:"tcp";framing:"raw"|"lines"|"length_be"|"length_le";max_frame_bytes:number;no_delay:boolean;idle_timeout_ms:number;message:TcpMessage}
