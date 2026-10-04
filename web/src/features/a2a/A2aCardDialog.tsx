import type { ErrorCopy } from "../../shared/i18n/errors";
import { LocalizedError, errorCopy } from "../../shared/i18n/errors";
import { t, useLanguage, translateCopy } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { Button, Callout, Dialog, Flex, TextField } from "@radix-ui/themes";
import { api } from "../../shared/api";
import { id } from "../../shared/model";
import { Editor, Field } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";
import type { A2aCardResult } from "./types";
import type { useA2aCard } from "./useA2aCard";
export default function A2aCardDialog({open,onOpenChange,source}:{open:boolean;onOpenChange:(value:boolean)=>void;source:ReturnType<typeof useA2aCard>}) {
  useLanguage();
  const state=useWorkbench();
  const [text,setText]=useState("{}");
  const [url,setUrl]=useState("");
  const [error,setError]=useState<ErrorCopy>("");
  const [pending,setPending]=useState<number|null>(null);
  const opened=useRef(open), epoch=useRef(0), mounted=useRef(true);
  const discovery=useRef<{workspace_id:string;discovery_id:string}|null>(null);
  const identity=JSON.stringify([state.authenticated,state.accountId,state.draft?.id,state.request?.id,state.draft?.data.active_environment_id,open]);
  function stop(){epoch.current++;setPending(null);const active=discovery.current;discovery.current=null;if(active)void api("/api/a2a/cards/discover/cancel","POST",active).catch(()=>{});}
  useEffect(()=>{return()=>stop();},[identity]);
  if(opened.current!==open){opened.current=open;epoch.current++;}
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;epoch.current++;};},[]);
  useEffect(()=>{if(open){setText(source.source||"{}");setUrl(state.request?.url||"");setError("");}},[open]);
  const busy=pending===epoch.current;
  async function validate(discover=false){
    if(busy || state.request?.protocol?.kind!=="a2a")return;
    const context=source.guard(),generation=epoch.current;
    const current=()=>mounted.current&&opened.current&&epoch.current===generation&&context();
    const discovery_id=discover?id():undefined;
    setPending(generation);setError("");
    try{
      if(state.dirty && !(await state.save(true)))throw new LocalizedError("请先解决工作区保存冲突");
      if(!current())return;
      const contextFields=source.context();
      if(discovery_id)discovery.current={workspace_id:contextFields.workspace_id,discovery_id};
      const result=await api<A2aCardResult>(discover?"/api/a2a/cards/discover":"/api/a2a/cards/import","POST",discover?{
        ...contextFields,discovery_id,request:{...state.request,url},
      }:{...contextFields,source:text,dialect:state.request.protocol.dialect});
      if(!current())return;
      source.attach(result);onOpenChange(false);
    }catch(caught){if(current())setError(errorCopy(caught));}
    finally{if(discovery.current?.discovery_id===discovery_id)discovery.current=null;if(mounted.current)setPending(value=>value===generation?null:value);}
  }
  return <Dialog.Root open={open} onOpenChange={onOpenChange}><Dialog.Content maxWidth="980px">
    <Dialog.Title>{t("A2A Agent Card 来源")}</Dialog.Title><Dialog.Description>{t("保留原始定义。导入不会发送消息；发现后也需明确选择调用端点。")}</Dialog.Description>
    <Flex gap="3" align="end" mt="4"><Field label="Agent Card URL"><TextField.Root disabled={busy} value={url} onChange={event=>setUrl(event.target.value)} placeholder="https://agent.example.com/.well-known/agent-card.json"/></Field><Button disabled={busy||!url} onClick={()=>void validate(true)}>{t("发现 Agent Card")}</Button>{busy&&<Button color="gray" variant="soft" onClick={stop}>{t("停止发现")}</Button>}</Flex>
    <Editor value={text} onChange={setText} readOnly={busy} dark={state.dark} jsonMode height="420px" label={t("Agent Card 原始 JSON")}/>
    {error&&<Callout.Root role="alert" color="red"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
    <Flex justify="end" gap="3" mt="4"><Dialog.Close><Button variant="soft" color="gray">{t("取消")}</Button></Dialog.Close><Button loading={busy} onClick={()=>void validate()}>{t("验证并使用定义")}</Button></Flex>
  </Dialog.Content></Dialog.Root>;
}
