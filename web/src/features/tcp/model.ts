import type { TcpConfig } from "./types";
export const tcpConfig=():TcpConfig=>({kind:"tcp",framing:"raw",max_frame_bytes:1048576,no_delay:true,idle_timeout_ms:0,message:{encoding:"text",payload_source:"",secret:false}});
