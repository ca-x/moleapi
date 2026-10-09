import {useEffect,useRef,useState} from "react";
import {parse} from "yaml";
import {Button,Callout,Dialog,Flex,Text,TextField} from "@radix-ui/themes";
import {api} from "../../shared/api";
import {t,useLanguage} from "../../shared/i18n";
import {Choice,Editor,Field} from "../../shared/ui";
import {id} from "../../shared/model";
import {useWorkbench} from "../workbench/context";
import {pickProjectSource,projectRelativePath} from "./projectInputs";
import type {WorkingFile} from "./projectInputs";
import {decodeProjectBytes} from "./saveProjectFile";

interface SourceFile {path:string;content:string}
export function checkedSourceFiles(files:WorkingFile[]):SourceFile[] {
 const selected=files.filter(file=>/\.(json|ya?ml|schema)$/i.test(file.path));
 if(!selected.length||selected.length>64)throw new Error("Source file count limit");
 const decoder=new TextDecoder("utf-8",{fatal:true,ignoreBOM:true});
 const result=selected.map(file=>({path:projectRelativePath(file.path),content:decoder.decode(decodeProjectBytes(file.encoding,file.content))}));
 if(new Set(result.map(file=>file.path)).size!==result.length||result.reduce((size,file)=>size+new TextEncoder().encode(file.content).length,0)>1024*1024)throw new Error("Source file size/path limit");
 return result;
}
export default function OpenapiSourceBundleDialog({open,onOpenChange}:{open:boolean;onOpenChange:(open:boolean)=>void}) {
 useLanguage();const state=useWorkbench();
 const [files,setFiles]=useState<SourceFile[]>([]),[entry,setEntry]=useState(""),[path,setPath]=useState(""),[name,setName]=useState(""),[busy,setBusy]=useState(false),[error,setError]=useState(false);
 const mounted=useRef(true),epoch=useRef(0),job=useRef<string|null>(null);
 const scope=JSON.stringify([state.accountId,state.authenticated,state.draft?.id,open]);const current=useRef(scope);current.current=scope;
 function cancel(){epoch.current++;const pending=job.current;job.current=null;setBusy(false);if(pending)void api("/api/generation/projects/cancel","POST",{job_id:pending}).catch(()=>{});}
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;epoch.current++;const pending=job.current;if(pending)void api("/api/generation/projects/cancel","POST",{job_id:pending}).catch(()=>{});};},[]);
 useEffect(()=>{epoch.current++;setFiles([]);setEntry("");setPath("");setName("");setError(false);setBusy(false);return()=>{epoch.current++;const pending=job.current;job.current=null;if(pending)void api("/api/generation/projects/cancel","POST",{job_id:pending}).catch(()=>{});};},[scope]);
 async function pick(kind:"directory"|"zip") {
  if(busy||!state.draft)return;
  const origin=scope,ticket=++epoch.current;
  const valid=()=>mounted.current&&current.current===origin&&epoch.current===ticket;
  setBusy(true);setError(false);
  try {
   const selected=await pickProjectSource(kind,valid);if(!selected||!valid())return;
   let values:WorkingFile[];
   if(selected.kind==="zip"){
    const jobId=id();job.current=jobId;
    const reply=await api<{files:WorkingFile[]}>("/api/generation/projects/import","POST",{workspace_id:state.draft.id,job_id:jobId,role:"working",source:selected});
    if(!valid())return;values=reply.files;
   } else values=selected.files;
   const files=checkedSourceFiles(values);if(!valid())return;
   setFiles(files);setPath(files[0].path);setEntry(files.find(file=>/(^|\/)openapi\.(json|ya?ml)$/i.test(file.path))?.path??files[0].path);
  }catch{if(valid())setError(true);}finally{if(valid()){job.current=null;setBusy(false);}}
 }
 function save(){
  if(busy||!files.length||!entry||!state.draft)return;
  try{
   const root=parse(files.find(file=>file.path===entry)!.content,{maxAliasCount:32});
   const version=typeof root?.openapi==="string"?root.openapi:"";
   if(!/^3\.[01]\./.test(version))throw new Error("OpenAPI entry missing");
   const source=JSON.stringify({format:"moleapi-openapi-source-v1",entry_file:entry,files});
   if(new TextEncoder().encode(source).length>1024*1024)throw new Error("Source envelope size limit");
   const spec={id:id(),name:name.trim()||t("OpenAPI 多文件定义"),kind:"openapi",dialect:version,source};
   state.updateData(data=>({...data,specifications:[...(data.specifications??[]),spec]}));
   onOpenChange(false);
  }catch{setError(true);}
 }
 const selected=files.find(file=>file.path===path);
 return <Dialog.Root open={open} onOpenChange={onOpenChange}><Dialog.Content className="openapi-source-bundle-dialog" maxWidth="1000px"><Dialog.Title>{t("OpenAPI 多文件源包")}</Dialog.Title><Dialog.Description>{t("读取目录或 ZIP 中的 JSON、YAML 和 schema 文件，选择入口定义。保留原始源文件，在生成项目时解析引用。")}</Dialog.Description>
 <Flex direction="column" gap="3" mt="3" aria-busy={busy}>
  <Field label={t("定义名称")}><TextField.Root value={name} maxLength={128} disabled={busy} onChange={event=>setName(event.target.value)}/></Field>
  <Flex gap="2" wrap="wrap"><Button highContrast variant="soft" disabled={busy} onClick={()=>void pick("directory")}>{t("读取源文件目录")}</Button><Button highContrast variant="soft" disabled={busy} onClick={()=>void pick("zip")}>{t("读取源文件 ZIP")}</Button>{busy&&<Button highContrast variant="soft" onClick={cancel}>{t("取消")}</Button>}</Flex>
  <Text size="1" color="gray">{t("最多 64 个源文件，源包总计 1 MiB。只解析提供的文件，不读取外部文件或联网；部分高级引用语义尚待补齐。")}</Text>
  {busy&&<Text role="status">{t("正在读取项目文件…")}</Text>}
  {error&&<Callout.Root color="red"><Callout.Text role="alert">{t("无法读取或保存源包，请检查 UTF8、文件路径、大小和入口的 OpenAPI 版本。")}</Callout.Text></Callout.Root>}
  {!!files.length&&<><Text size="2" role="status">{t("已读取 {{count}} 个源文件。",{count:files.length})}</Text><Field label={t("OpenAPI 入口文件")}><Choice label={t("OpenAPI 入口文件")} value={entry} onChange={setEntry} options={files.map(file=>({value:file.path,label:file.path}))}/></Field><Field label={t("预览源文件")}><Choice label={t("预览源文件")} value={path} onChange={setPath} options={files.map(file=>({value:file.path,label:file.path}))}/></Field>{selected&&<Editor value={selected.content} readOnly dark={state.dark} jsonMode={path.endsWith(".json")} height="35vh"/>}</>}
  <Flex gap="2" justify="end" wrap="wrap"><Dialog.Close><Button highContrast variant="soft">{t("关闭")}</Button></Dialog.Close><Button disabled={busy||!files.length||!entry} onClick={save}>{t("保存多文件定义")}</Button></Flex>
 </Flex></Dialog.Content></Dialog.Root>;
}
