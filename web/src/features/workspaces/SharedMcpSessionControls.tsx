import { useState } from "react";
import { Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { RequestSpec } from "../../shared/types";
import McpClient, { type McpChannel } from "../mcp/McpClient";
export default function SharedMcpSessionControls(props:{request:RequestSpec;channel:McpChannel;dark:boolean;isCurrent:()=>boolean}) {
  useLanguage();const [request,setRequest]=useState(()=>structuredClone(props.request));
  return <section className="shared-mcp" aria-label={t("共享 MCP 实时调用")}><Text as="p" size="1" color="gray">{t("能力选择与参数修改仅用于当前连接，不会自动保存到共享请求。")}</Text><McpClient scope={props.channel.session?.id??""} request={request} channel={props.channel} dark={props.dark} isCurrent={props.isCurrent} connectionSettings={false} onChange={patch=>{if(props.isCurrent())setRequest(current=>({...current,...patch}));}}/></section>;
}
