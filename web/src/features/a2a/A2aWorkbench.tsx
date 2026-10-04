import { t, useLanguage, message } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { Badge, Button, Callout, Flex, Tabs, Text, TextField } from "@radix-ui/themes";
import { FileCode, RefreshCw, Send, Square } from "lucide-react";
import { Choice, Editor, Field } from "../../shared/ui";
import { id } from "../../shared/model";
import { useWorkbench } from "../workbench/context";
import SessionEventPane from "../protocols/SessionEventPane";
import type { A2aConfig, A2aMethod, A2aInterface } from "./types";
import { paramsTemplate, interfaceDialect } from "./model";
import { useA2aCard } from "./useA2aCard";
import A2aCardDialog from "./A2aCardDialog";
import A2aContent from "./A2aContent";
import { object } from "../mcp/model";
const operations:{value:A2aMethod;label:string}[]=[
  {value:"message/send",get label() { return t("发送消息"); }},{value:"message/stream",get label() { return t("消息流"); }},{value:"tasks/get",get label() { return t("读取任务"); }},{value:"tasks/cancel",get label() { return t("取消远端任务"); }},{value:"tasks/resubscribe",get label() { return t("重新订阅任务"); }},{value:"tasks/list",get label() { return t("列出任务"); }},
  {value:"tasks/pushNotificationConfig/set",get label() { return t("设置任务推送"); }},{value:"tasks/pushNotificationConfig/get",get label() { return t("读取任务推送"); }},{value:"tasks/pushNotificationConfig/list",get label() { return t("列出任务推送"); }},{value:"tasks/pushNotificationConfig/delete",get label() { return t("删除任务推送"); }},
];
export default function A2aWorkbench(){
  useLanguage();
  const state=useWorkbench(),source=useA2aCard();
  const latest=useRef(state);latest.current=state;
  const channel=state.protocolSession;
  const {session,events,send,close,busy,sending,error,dropped}=channel;
  const config=state.request?.protocol?.kind==="a2a"?state.request.protocol:null;
  const identity=JSON.stringify([state.authenticated,state.accountId,state.draft?.id,state.request?.id,state.draft?.data.active_environment_id,session?.id]);
  const scope=useRef(identity);scope.current=identity;
  const mounted=useRef(true);
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  const [tab,setTab]=useState("request"),[sourceOpen,setSourceOpen]=useState(false),[pending,setPending]=useState<string[]>([]),[resultCursor,setResultCursor]=useState<number|null>(null);
  const pane=useRef<HTMLElement>(null);
  const open=session?.state==="open",active=!!session&&["connecting","open"].includes(session.state);
  function reveal(value:string){setTab(value);if(pane.current)pane.current.scrollTop=0;}
  useEffect(()=>{setPending([]);setSourceOpen(false);setResultCursor(null);if(pane.current)pane.current.scrollTop=0;},[identity]);
  useEffect(()=>{reveal("request");},[state.accountId,state.draft?.id,state.request?.id]);
  useEffect(()=>{
    const finished=new Set(events.flatMap(event=>event.message.kind==="a2a_finished"||event.message.kind==="a2a_error"?[event.message.request_id]:[]));
    setPending(values=>values.filter(value=>!finished.has(value)));
  },[events]);
  function update(patch:Partial<A2aConfig>){const current=latest.current.request?.protocol;if(current?.kind==="a2a")latest.current.updateRequest({protocol:{...current,...patch}});}
  async function run(method?:A2aMethod,params?:string){
    const current=latest.current.request?.protocol;if(current?.kind!=="a2a"||!open)return;
    const ticket=id(),origin=identity;setPending(values=>[...values,ticket]);
    const sent=await send({kind:"a2a_request",request_id:ticket,method:method||current.operation,params_source:params||current.params_source});
    if(mounted.current&&scope.current===origin){if(!sent)setPending(values=>values.filter(value=>value!==ticket));else reveal("results");}
  }
  useEffect(()=>{
    const dispatch=()=>{if(open)void run();else void latest.current.protocolSession.connect();};
    state.a2aRun.current=dispatch;
    return()=>{if(latest.current.a2aRun.current===dispatch)latest.current.a2aRun.current=null;};
  },[identity,open]);
  function template(dialect=config?.dialect,method=config?.operation){
    if(!dialect||!method)return;
    const current=source.guard(),draft=paramsTemplate(dialect,method);
    state.setGuard({title:message("使用 A2A 参数模板"),description:message("当前 JSON 参数将被所选版本与操作的示例替换。原始 Agent Card 保留。"),action:()=>{if(current())update({params_source:draft});}});
  }
  function adopt(entry:A2aInterface){
    if(!entry.supported||active||busy)return;
    const current=source.guard();
    state.setGuard({title:message("使用 Agent Card 接口"),description:message("请求端点将改为 {{url}}。确认所选版本、鉴权和参数后再连接。", { url: entry.url }),action:()=>{
      if(!current())return;
      const currentConfig=latest.current.request?.protocol;if(currentConfig?.kind!=="a2a")return;
      const dialect=interfaceDialect(entry.version);
      if(!dialect)return;
      const transport=entry.transport.toLowerCase().includes("jsonrpc")?"jsonrpc":"http-json";
      latest.current.updateRequest({url:entry.url,protocol:{...currentConfig,interface_url:entry.url,dialect,transport}});
    }});
  }
  const results=events.filter(event=>event.message.kind==="a2a_result"||event.message.kind==="a2a_stream"||event.message.kind==="a2a_error");
  const selected=results.find(event=>event.cursor===resultCursor)||results.at(-1);
  const result=selected?.message.kind==="a2a_result"||selected?.message.kind==="a2a_stream"?selected.message.result:null;
  const value=object(result),task=object(value?.task)||object(value?.statusUpdate)||(value?.status?value:null);
  const taskId=typeof task?.id==="string"?task.id:typeof task?.taskId==="string"?task.taskId:"";
  if(!config)return null;
  return <section className="a2a-workbench" aria-label={t("A2A 客户端")} ref={pane}>
    <div className="a2a-toolbar"><Flex gap="3" align="center" wrap="wrap">
      <Choice value={config.dialect} label={t("A2A 版本")} disabled={active||busy} onChange={dialect=>update({dialect,...(dialect==="0.3"?{transport:"jsonrpc" as const}:{})})} options={[{value:"0.3",label:"A2A 0.3"},{value:"1.0",label:"A2A 1.0"}]}/>
      <Choice value={config.transport} label={t("A2A 传输")} disabled={active||busy} onChange={transport=>update({transport})} options={[{value:"jsonrpc",label:"JSON-RPC"},...(config.dialect==="1.0"?[{value:"http-json" as const,label:"HTTP+JSON"}]:[])]}/>
      <Button size="1" variant="soft" color="gray" onClick={()=>setSourceOpen(true)} disabled={active||busy}><FileCode size={14}/>{t("Agent Card 来源")}</Button>
      <Badge color={open?"green":"gray"}>{session?.state||t("未连接")}</Badge>
      <Button size="1" variant="soft" color="gray" disabled={!active&&!busy} onClick={()=>void close()}><Square size={14}/>{t("停止本地会话")}</Button>
    </Flex></div>
    {(error||session?.reason||source.error)&&<Callout.Root role="alert" color="red"><Callout.Text>{error||session?.reason||source.error}</Callout.Text></Callout.Root>}
    <Tabs.Root value={tab} onValueChange={reveal} className="a2a-tabs"><Tabs.List><Tabs.Trigger value="request">{t("消息与任务")}</Tabs.Trigger><Tabs.Trigger value="results">{t("响应与产物")}</Tabs.Trigger><Tabs.Trigger value="events">{t("任务事件")}</Tabs.Trigger><Tabs.Trigger value="card">Agent Card</Tabs.Trigger></Tabs.List>
      <Tabs.Content value="request"><Flex gap="3" align="center" wrap="wrap">
        <Choice value={config.operation} label={t("A2A 操作")} onChange={operation=>update({operation})} options={operations.filter(value=>config.dialect==="1.0"||value.value!=="tasks/list")}/>
        <Button size="1" variant="soft" color="gray" onClick={()=>template()}><RefreshCw size={14}/>{t("使用参数模板")}</Button><Button disabled={!open||sending} onClick={()=>void run()}><Send size={14}/>{t("运行")}</Button>
      </Flex><Text size="1" color="gray">{t("参数按所选版本保留原文；任务取消与停止本地请求是独立操作。推送注册需要明确的地址和操作。")}</Text><Editor value={config.params_source} onChange={params_source=>update({params_source})} dark={state.dark} jsonMode height="100%" label={t("A2A JSON 参数")}/></Tabs.Content>
      <Tabs.Content value="results">{pending.length>0&&<Flex gap="2" wrap="wrap">{pending.map(ticket=><Button size="1" key={ticket} variant="soft" color="gray" disabled={!open||sending} onClick={()=>void send({kind:"a2a_stop",request_id:ticket})}><Square size={14}/>{t("停止请求")} {ticket.slice(0,8)}</Button>)}</Flex>}
        <Flex gap="2" wrap="wrap">{results.slice(-64).reverse().map(event=><Button size="1" key={event.cursor} color="gray" variant={selected?.cursor===event.cursor?"soft":"ghost"} onClick={()=>setResultCursor(event.cursor)}>{"method" in event.message?event.message.method:""} · #{event.cursor}</Button>)}</Flex>
        {taskId&&<Flex gap="3" align="end" wrap="wrap"><Field label={t("返回的任务 ID")}><TextField.Root value={taskId} readOnly/></Field><Button size="1" variant="soft" color="gray" disabled={!open||sending} onClick={()=>void run("tasks/get",JSON.stringify({id:taskId,historyLength:20}))}>{t("读取任务")}</Button><Button size="1" variant="outline" color="red" disabled={!open||sending} onClick={()=>{const origin=source.guard();state.setGuard({title:message("取消远端 A2A 任务"),description:message("向 Agent 发送任务取消请求：{{task}}", { task: taskId }),action:()=>{if(origin())void run("tasks/cancel",JSON.stringify({id:taskId}));}});}}>{t("取消远端任务")}</Button></Flex>}
        {selected?.message.kind==="a2a_error"?<Editor value={JSON.stringify(selected.message.error,null,2)} dark={state.dark} jsonMode readOnly height="280px" label={t("A2A 错误")}/>:result?<A2aContent result={result} dark={state.dark}/>:<Text color="gray">{t("运行消息或任务操作后，响应会显示在这里。")}</Text>}
      </Tabs.Content>
      <Tabs.Content value="events"><SessionEventPane events={events} dropped={dropped} dark={state.dark} sessionId={session?.id} protocolLabel="A2A"/></Tabs.Content>
      <Tabs.Content value="card">{source.busy&&<Text role="status">{t("正在读取 Agent Card…")}</Text>}{source.card?.warnings.map((warning,index)=><Text key={index} color="amber">{warning}</Text>)}
        {source.card?.interfaces.map((entry,index)=><Flex gap="3" align="center" wrap="wrap" key={index}><Badge color={entry.supported?"gray":"amber"}>{entry.version} · {entry.transport}</Badge><Text size="1" className="mono">{entry.url}</Text><Button size="1" variant="soft" disabled={!entry.supported||active||busy} onClick={()=>adopt(entry)}>{t("使用此接口")}</Button></Flex>)}
        <Editor value={source.source||"{}"} dark={state.dark} jsonMode readOnly height="100%" label={t("保存的 Agent Card 原文")}/>
      </Tabs.Content>
    </Tabs.Root>
    <A2aCardDialog open={sourceOpen} onOpenChange={setSourceOpen} source={source}/>
  </section>;
}
