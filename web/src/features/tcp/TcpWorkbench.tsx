import TcpConnectionSettings from "./TcpConnectionSettings";
import TcpMessageEditor from "./TcpMessageEditor";
import { t, useLanguage, message } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { Badge, Button, Callout, Flex, Tabs, Text } from "@radix-ui/themes";
import { Send, Square, ArrowRightToLine } from "lucide-react";
import { useWorkbench } from "../workbench/context";
import SessionEventPane from "../protocols/SessionEventPane";
import type { TcpConfig } from "./types";
import { bytes } from "../../shared/model";
export default function TcpWorkbench() {
  useLanguage();
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
    state.setGuard({title:message("半关闭 TCP 发送端"),description:message("发送 EOF 后继续接收对端数据。本次连接不能再发送报文。"),action:()=>{if(mounted.current&&scope.current===origin)void send({kind:"tcp_half_close"});}});
  }
  const problem=error||(session?.state==="error"?session.reason:"");
  if(!config)return null;
  return <section className="tcp-workbench" aria-label={t("TCP 客户端")}>
    <Flex align="center" justify="between" gap="3" wrap="wrap" className="tcp-toolbar">
      <Flex align="center" gap="3" wrap="wrap"><Badge color={open?"green":"gray"}>{session?.state||t("未连接")}</Badge>{session&&<Text size="1" className="mono wrap-anywhere">{session.url} {t("· 线缆字节 ↓")}{bytes(session.received_bytes)} ↑{bytes(session.sent_bytes)}</Text>}{half&&<Badge color="gray">{t("发送端已半关闭")}</Badge>}</Flex>
      <Flex gap="3"><Button size="1" variant="soft" color="gray" disabled={!open||half||sending} onClick={halfClose}><ArrowRightToLine size={14}/>{t("半关闭")}</Button><Button size="1" variant="soft" color="gray" disabled={!active} onClick={()=>void close()}><Square size={14}/>{t("断开")}</Button></Flex>
    </Flex>
    {problem&&<Callout.Root color="red" role="alert"><Callout.Text>{problem}</Callout.Text></Callout.Root>}{!problem&&session?.reason&&<Text size="1" color="gray">{session.reason}</Text>}
    <Tabs.Root value={tab} onValueChange={setTab} className="tcp-tabs"><Tabs.List><Tabs.Trigger value="payload">{t("发送报文")}</Tabs.Trigger><Tabs.Trigger value="events">{t("收发记录")}</Tabs.Trigger><Tabs.Trigger value="settings">{t("连接与分帧")}</Tabs.Trigger></Tabs.List>
      <Tabs.Content value="payload"><TcpMessageEditor message={config.message} onChange={message=>change({message})} dark={state.dark}><Button disabled={!open||half||sending} loading={sending} onClick={()=>void transmit()}><Send size={15}/>{t("发送报文")}</Button></TcpMessageEditor></Tabs.Content>
      <Tabs.Content value="events"><SessionEventPane events={events} dropped={dropped} dark={state.dark} sessionId={session?.id} protocolLabel="TCP"/></Tabs.Content>
      <Tabs.Content value="settings"><TcpConnectionSettings request={state.request!} scope={identity} onChange={state.updateRequest} disabled={active}/></Tabs.Content>
    </Tabs.Root>
  </section>;
}
