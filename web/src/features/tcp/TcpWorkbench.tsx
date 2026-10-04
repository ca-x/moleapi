import { useEffect, useRef, useState } from "react";
import { Badge, Button, Callout, Checkbox, Flex, Tabs, Text, TextField } from "@radix-ui/themes";
import { Send, Square, ArrowRightToLine } from "lucide-react";
import { Choice, Editor, Field } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";
import SessionEventPane from "../protocols/SessionEventPane";
import type { TcpConfig } from "./types";
import { bytes } from "../../shared/model";
export default function TcpWorkbench() {
  const state=useWorkbench(),channel=state.protocolSession;
  const {session,events,send,close,busy,sending,error,dropped}=channel;
  const config=state.request?.protocol?.kind==="tcp"?state.request.protocol:null;
  const latest=useRef(state);latest.current=state;
  const identity=JSON.stringify([state.authenticated,state.accountId,state.draft?.id,state.request?.id,state.draft?.data.active_environment_id,session?.id]);
  const scope=useRef(identity);scope.current=identity;
  const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  const [tab,setTab]=useState("payload");
  useEffect(()=>{setTab("payload");},[state.accountId,state.draft?.id,state.request?.id]);
  const active=busy||session?.state==="open"||session?.state==="connecting",open=session?.state==="open",half=session?.client_half_closed===true;
  function change(patch:Partial<TcpConfig>) {const current=latest.current.request?.protocol;if(current?.kind==="tcp")latest.current.updateRequest({protocol:{...current,...patch}});}
  async function transmit() {
    if(!config||!open||half||sending)return;
    const origin=identity;
    const sent=await send({kind:"tcp_send",message:structuredClone(config.message)});
    if(sent&&mounted.current&&scope.current===origin)setTab("events");
  }
  function halfClose() {
    const origin=identity;
    state.setGuard({title:"半关闭 TCP 发送端",description:"发送 EOF 后继续接收对端数据。本次连接不能再发送报文。",action:()=>{if(mounted.current&&scope.current===origin)void send({kind:"tcp_half_close"});}});
  }
  const problem=error||(session?.state==="error"?session.reason:"");
  if(!config)return null;
  return <section className="tcp-workbench" aria-label="TCP 客户端">
    <Flex align="center" justify="between" gap="3" wrap="wrap" className="tcp-toolbar">
      <Flex align="center" gap="3" wrap="wrap"><Badge color={open?"green":"gray"}>{session?.state||"未连接"}</Badge>{session&&<Text size="1" className="mono wrap-anywhere">{session.url} · 线缆字节 ↓{bytes(session.received_bytes)} ↑{bytes(session.sent_bytes)}</Text>}{half&&<Badge color="gray">发送端已半关闭</Badge>}</Flex>
      <Flex gap="3"><Button size="1" variant="soft" color="gray" disabled={!open||half||sending} onClick={halfClose}><ArrowRightToLine size={14}/>半关闭</Button><Button size="1" variant="soft" color="gray" disabled={!active} onClick={()=>void close()}><Square size={14}/>断开</Button></Flex>
    </Flex>
    {problem&&<Callout.Root color="red" role="alert"><Callout.Text>{problem}</Callout.Text></Callout.Root>}{!problem&&session?.reason&&<Text size="1" color="gray">{session.reason}</Text>}
    <Tabs.Root value={tab} onValueChange={setTab} className="tcp-tabs"><Tabs.List><Tabs.Trigger value="payload">发送报文</Tabs.Trigger><Tabs.Trigger value="events">收发记录</Tabs.Trigger><Tabs.Trigger value="settings">连接与分帧</Tabs.Trigger></Tabs.List>
      <Tabs.Content value="payload"><Flex gap="3" align="center" wrap="wrap"><Choice value={config.message.encoding} label="TCP 报文编码" options={[{value:"text",label:"UTF8 文本"},{value:"hex",label:"Hex"},{value:"base64",label:"Base64"}]} onChange={encoding=>change({message:{...config.message,encoding}})}/><Text as="label" size="2"><Flex gap="2" align="center"><Checkbox checked={config.message.secret} onCheckedChange={value=>change({message:{...config.message,secret:value===true}})}/>私密报文</Flex></Text><Button disabled={!open||half||sending} loading={sending} onClick={()=>void transmit()}><Send size={15}/>发送报文</Button></Flex>
        <Text size="1" color="gray">仅在点击发送时解析本次连接的环境快照；环境修改后重连生效。存在私密变量或发送私密报文后，本次会话的字节载荷会保守隐藏，仍记录方向、大小和状态。</Text>
        <Editor value={config.message.payload_source} onChange={payload_source=>change({message:{...config.message,payload_source}})} dark={state.dark} height="100%" label="TCP 报文内容"/>
      </Tabs.Content>
      <Tabs.Content value="events"><SessionEventPane events={events} dropped={dropped} dark={state.dark} sessionId={session?.id} protocolLabel="TCP"/></Tabs.Content>
      <Tabs.Content value="settings"><Flex gap="4" wrap="wrap"><Field label="分帧方式"><Choice value={config.framing} label="TCP 分帧方式" disabled={active} options={[{value:"raw",label:"原始字节流"},{value:"lines",label:"UTF8 行（LF / CRLF）"},{value:"length_be",label:"4 字节长度前缀 · 大端"},{value:"length_le",label:"4 字节长度前缀 · 小端"}]} onChange={framing=>change({framing})}/></Field><Field label="连接超时（ms）"><TextField.Root type="number" min="1" max="120000" value={state.request?.timeout_ms} disabled={active} onChange={event=>state.updateRequest({timeout_ms:Number(event.target.value)})}/></Field><Field label="单帧上限（字节）"><TextField.Root type="number" min="1" max="1048576" value={config.max_frame_bytes} disabled={active} onChange={event=>change({max_frame_bytes:Number(event.target.value)})}/></Field><Field label="空闲超时（ms，0 为关闭）"><TextField.Root type="number" min="0" max="120000" value={config.idle_timeout_ms} disabled={active} onChange={event=>change({idle_timeout_ms:Number(event.target.value)})}/></Field></Flex>
        <Text as="label" size="2"><Flex gap="2" align="center"><Checkbox checked={state.request?.verify_tls===true} disabled={active} onCheckedChange={value=>state.updateRequest({verify_tls:value===true})}/>验证 TLS 证书</Flex></Text>
        <Text as="label" size="2"><Flex gap="2" align="center"><Checkbox checked={config.no_delay} disabled={active} onCheckedChange={value=>change({no_delay:value===true})}/>禁用 Nagle（TCP_NODELAY）</Flex></Text>
        <Text as="p" size="2" color="gray">tcp://host:port 或 tcps://host:port；TLS 证书验证在此处设置。原始模式展示读取块，不代表对端报文边界。行分帧追加 LF 并去除接收的 LF／CRLF；长度分帧由编解码器添加和读取 4 字节前缀。</Text><Text as="p" size="2" color="gray">默认导出隐藏私密或无法可靠筛查的二进制草稿；保留完整原文请显式选择包含私密值导出。</Text><Text as="p" size="2" color="gray">HTTP 鉴权、请求头、查询参数及响应脚本不用于 TCP。连接前脚本沿用项目、集合和请求设置。</Text>
      </Tabs.Content>
    </Tabs.Root>
  </section>;
}
