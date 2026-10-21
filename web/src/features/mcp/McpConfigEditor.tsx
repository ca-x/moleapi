import { errorCopy, type ErrorCopy } from "../../shared/i18n/errors";
import { t, useLanguage, translateCopy } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { AlertDialog, Button, Callout, Checkbox, Dialog, Flex, Text, TextField } from "@radix-ui/themes";
import { Choice, Editor, Field } from "../../shared/ui";
import type { RequestSpec } from "../../shared/types";
import { saveFile } from "../../shared/api";
import { hostConfig, parseHostConfig, type McpHostEntry } from "./config";
export default function McpConfigEditor(props:{scope:string;open:boolean;onOpenChange:(open:boolean)=>void;request:RequestSpec;dark:boolean;isCurrent:()=>boolean;onChange:(patch:Partial<RequestSpec>)=>void;readSavedRequest:(includeSecrets:boolean,name:string)=>Promise<RequestSpec>;disabled?:boolean;exportDisabled?:boolean;confirm?:(action:()=>void)=>void}) {
  useLanguage();const latest=useRef(props);latest.current=props;
  const [text,setText]=useState("{}"),[entries,setEntries]=useState<McpHostEntry[]>([]),[selected,setSelected]=useState(""),[name,setName]=useState("MoleAPI");
  const [error,setError]=useState<ErrorCopy>(""),[includeSecrets,setIncludeSecrets]=useState(false),[exporting,setExporting]=useState(false);
  const scope=JSON.stringify([props.scope,props.open]);const identity=useRef(scope),epoch=useRef(0);if(identity.current!==scope){identity.current=scope;epoch.current++;}
  const renderEpoch=epoch.current;const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  const source=JSON.stringify([text,selected]),sourceRef=useRef(source);sourceRef.current=source;
  const form=JSON.stringify([name,includeSecrets]),formRef=useRef(form);formRef.current=form;
  const [confirmation,setConfirmation]=useState<{action:()=>void;scope:string}|null>(null);
  function current(){return mounted.current&&identity.current===scope&&epoch.current===renderEpoch&&props.open&&!latest.current.disabled&&props.isCurrent();}
  useEffect(()=>{if(!props.open)return;const config=props.request.protocol;setText(config?.kind==="mcp"&&config.config_source?config.config_source:"{}");setExporting(false);setIncludeSecrets(false);setName(props.request.name||"MoleAPI");setEntries([]);setSelected("");setError("");setConfirmation(null);},[props.open]);
  function validate(){if(!current())return;try{const values=parseHostConfig(text);setEntries(values);setSelected(values[0].name);setError("");}catch(caught){setError(errorCopy(caught));setEntries([]);}}
  function apply(){
    if(!current())return;const entry=entries.find(value=>value.name===selected);if(!entry||props.request.protocol?.kind!=="mcp")return;
    const request=JSON.stringify(props.request),origin=source;
    const commit=()=>{if(!current()||sourceRef.current!==origin||JSON.stringify(latest.current.request)!==request)return;const config=latest.current.request.protocol;if(config?.kind!=="mcp")return;latest.current.onChange({url:entry.url,headers:entry.headers,auth:{kind:"none",token:"",username:"",password:""},protocol:{...config,...entry.config,operation:config.operation,name:config.name,arguments_source:config.arguments_source,uri:config.uri}});props.onOpenChange(false);};
    if(props.confirm)props.confirm(commit);else setConfirmation({action:commit,scope});
  }
  async function exportCurrent(){
    if(!current()||exporting||props.exportDisabled)return;const origin=form;setExporting(true);setError("");
    try{const request=await props.readSavedRequest(includeSecrets,name);if(!current()||latest.current.exportDisabled||formRef.current!==origin)return;await saveFile({filename:"moleapi-mcp.json",mime:"application/json",content:hostConfig(request,request.name)},()=>current()&&!latest.current.exportDisabled&&formRef.current===origin);}
    catch(caught){if(current())setError(errorCopy(caught));}finally{if(current())setExporting(false);}
  }
  return <><Dialog.Root open={props.open} onOpenChange={props.onOpenChange}><Dialog.Content maxWidth="900px">
    <Dialog.Title>{t("MCP Host 配置")}</Dialog.Title><Dialog.Description>{t("粘贴 mcpServers 配置或单个服务定义。验证和导入不会连接服务。")}</Dialog.Description>
    <Editor value={text} onChange={value=>{if(current()){setText(value);setEntries([]);}}} dark={props.dark} readOnly={props.disabled} jsonMode height="360px" label="MCP Host JSON"/>
    {error&&<Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
    <Flex gap="3" align="center" mt="3"><Button variant="soft" disabled={props.disabled} onClick={validate}>{t("验证配置")}</Button>{entries.length>0&&<Choice label={t("MCP Host 服务")} value={selected} onChange={value=>{if(current())setSelected(value);}} options={entries.map(entry=>({value:entry.name,label:entry.name}))}/>}</Flex>
    <Flex gap="3" align="end" mt="4"><Field label={t("Host 服务名称")}><TextField.Root value={name} disabled={exporting||props.disabled} onChange={event=>{if(current())setName(event.target.value);}}/></Field><Button variant="soft" color="gray" disabled={props.disabled||props.exportDisabled} loading={exporting} onClick={()=>void exportCurrent()}>{t("导出 Host 配置")}</Button></Flex>
    <label className="checkbox-label"><Checkbox checked={includeSecrets} onCheckedChange={value=>{if(current())setIncludeSecrets(value===true);}} disabled={exporting||props.disabled}/>{t("包含密钥值")}</label>
    <Text size="1" color="gray">{t("默认排除私密值。完整 Host 配置包含鉴权与环境变量，请妥善保管。")}</Text>
    <Flex gap="3" justify="end" mt="4"><Dialog.Close><Button variant="soft" color="gray">{t("关闭")}</Button></Dialog.Close><Button disabled={props.disabled||!entries.length} onClick={apply}>{t("使用所选配置")}</Button></Flex>
  </Dialog.Content></Dialog.Root>
  <AlertDialog.Root open={confirmation?.scope===scope} onOpenChange={open=>{if(!open)setConfirmation(null);}}><AlertDialog.Content><AlertDialog.Title>{t("使用 MCP Host 配置")}</AlertDialog.Title><AlertDialog.Description>{t("当前连接配置与鉴权会被替换，请求参数草稿保留。导入后可检查配置，再手动连接。")}</AlertDialog.Description><Flex gap="3" mt="4" justify="end"><AlertDialog.Cancel><Button variant="soft" color="gray">{t("取消")}</Button></AlertDialog.Cancel><Button onClick={()=>{const action=confirmation?.action;setConfirmation(null);action?.();}}>{t("使用所选配置")}</Button></Flex></AlertDialog.Content></AlertDialog.Root></>;
}
