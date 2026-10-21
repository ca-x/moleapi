import type { ErrorCopy } from "../../shared/i18n/errors";
import { errorCopy } from "../../shared/i18n/errors";
import { t, useLanguage, translateCopy } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { Button, Callout, Dialog, Flex, TextField } from "@radix-ui/themes";
import { id } from "../../shared/model";
import { Editor, Field } from "../../shared/ui";
import type { A2aCardResult } from "./types";
import type { RequestSpec } from "../../shared/types";
export interface A2aCardEditorProps {scope:string;open:boolean;onOpenChange:(value:boolean)=>void;request:RequestSpec;source?:string;dark:boolean;disabled?:boolean;isCurrent:()=>boolean;onAttach:(value:A2aCardResult)=>void;importCard:(source:string)=>Promise<A2aCardResult>;discoverCard:(url:string,id:string)=>Promise<A2aCardResult>;cancelDiscovery:(id:string)=>Promise<unknown>;beforeRead?:()=>Promise<void>}
export default function A2aCardEditor(state:A2aCardEditorProps) {
  useLanguage();const {open,onOpenChange}=state;const latest=useRef(state);latest.current=state;
  const [text,setText]=useState("{}");
  const [url,setUrl]=useState("");
  const [error,setError]=useState<ErrorCopy>("");
  const [pending,setPending]=useState<number|null>(null);
  const opened=useRef(open), epoch=useRef(0), mounted=useRef(true);
  const discovery=useRef<{id:string;cancel:(id:string)=>Promise<unknown>}|null>(null);
  const identity=JSON.stringify([state.scope,state.request,open]);const scope=useRef(identity);if(scope.current!==identity){scope.current=identity;epoch.current++;}
  function stop(){epoch.current++;setPending(null);const active=discovery.current;discovery.current=null;if(active)void active.cancel(active.id).catch(()=>{});}
  useEffect(()=>{return()=>stop();},[identity]);
  if(opened.current!==open){opened.current=open;epoch.current++;}
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;epoch.current++;};},[]);
  useEffect(()=>{if(open){setText(state.source||"{}");setUrl(state.request?.url||"");setError("");}},[open,state.scope]);
  const busy=pending===epoch.current;
  const form=JSON.stringify([text,url]),formRef=useRef(form);formRef.current=form;
  function admitted(){return mounted.current&&scope.current===identity&&opened.current&&!latest.current.disabled&&latest.current.isCurrent();}
  async function validate(discover=false){
    if(!admitted()||busy||state.request.protocol?.kind!=="a2a")return;
    const origin=form,generation=epoch.current;
    const current=()=>admitted()&&epoch.current===generation&&formRef.current===origin;
    const discovery_id=discover?id():undefined;
    setPending(generation);setError("");
    try{
      if(state.beforeRead)await state.beforeRead();
      if(!current())return;
      if(discovery_id)discovery.current={id:discovery_id,cancel:state.cancelDiscovery};
      const result=await (discover?state.discoverCard(url,discovery_id!):state.importCard(text));
      if(!current())return;
      state.onAttach(result);onOpenChange(false);
    }catch(caught){if(current())setError(errorCopy(caught));}
    finally{if(discovery.current?.id===discovery_id)discovery.current=null;if(mounted.current)setPending(value=>value===generation?null:value);}
  }
  return <Dialog.Root open={open} onOpenChange={onOpenChange}><Dialog.Content maxWidth="980px">
    <Dialog.Title>{t("A2A Agent Card 来源")}</Dialog.Title><Dialog.Description>{t("保留原始定义。导入不会发送消息；发现后也需明确选择调用端点。")}</Dialog.Description>
    <Flex gap="3" align="end" mt="4"><Field label="Agent Card URL"><TextField.Root disabled={busy||state.disabled} value={url} onChange={event=>{if(admitted())setUrl(event.target.value);}} placeholder="https://agent.example.com/.well-known/agent-card.json"/></Field><Button disabled={busy||state.disabled||!url} onClick={()=>void validate(true)}>{t("发现 Agent Card")}</Button>{busy&&<Button color="gray" variant="soft" onClick={stop}>{t("停止发现")}</Button>}</Flex>
    <Editor value={text} onChange={value=>{if(admitted())setText(value);}} readOnly={busy||state.disabled} dark={state.dark} jsonMode height="420px" label={t("Agent Card 原始 JSON")}/>
    {error&&<Callout.Root role="alert" color="red"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
    <Flex justify="end" gap="3" mt="4"><Dialog.Close><Button variant="soft" color="gray">{t("取消")}</Button></Dialog.Close><Button disabled={state.disabled} loading={busy} onClick={()=>void validate()}>{t("验证并使用定义")}</Button></Flex>
  </Dialog.Content></Dialog.Root>;
}
