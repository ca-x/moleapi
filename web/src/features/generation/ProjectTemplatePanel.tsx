import {useEffect,useRef,useState} from "react";
import {z} from "zod";
import {Button,Callout,Flex,Text} from "@radix-ui/themes";
import {api,pickFile} from "../../shared/api";
import {t} from "../../shared/i18n";
import {Choice,Editor,Field} from "../../shared/ui";
import {id} from "../../shared/model";
import {pickProjectSource,projectRelativePath} from "./projectInputs";
import {decodeProjectBytes} from "./saveProjectFile";
import type {ProjectFile} from "./projectTypes";
const schema=z.object({format:z.literal("moleapi-codegen-templates-v1"),files:z.array(z.object({path:z.string().max(256),content:z.string().max(64*1024)}).strict()).min(1).max(128)}).strict();
export type TemplateBundle=z.infer<typeof schema>;
export function parseTemplateBundle(value:unknown):TemplateBundle {
 const bundle=schema.parse(value);
 if(new TextEncoder().encode(JSON.stringify(bundle)).length>512*1024)throw new Error("template limit");
 const paths=new Set<string>();
 for(const file of bundle.files){projectRelativePath(file.path);if(!file.path.endsWith(".mustache")||paths.has(file.path)||new TextEncoder().encode(file.content).length>64*1024)throw new Error("invalid template");paths.add(file.path);}
 return bundle;
}
export default function ProjectTemplatePanel({workspaceId,scope,value,disabled,dark,onChange,onBusyChange}:{workspaceId:string;scope:string;value:TemplateBundle|null;disabled:boolean;dark:boolean;onChange:(bundle:TemplateBundle|null)=>void;onBusyChange:(busy:boolean)=>void}){
 const [pending,setPending]=useState(false),[error,setError]=useState(false),[path,setPath]=useState(value?.files[0]?.path??"");
 const current=useRef(scope);current.current=scope;const mounted=useRef(true),job=useRef<string|null>(null),generation=useRef(0);
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;onBusyChange(false);const previous=job.current;if(previous)void api("/api/generation/projects/cancel","POST",{job_id:previous}).catch(()=>{});};},[]);
 const selected=value?.files.find(file=>file.path===path)??value?.files[0];
 function stop(){generation.current++;const previous=job.current;job.current=null;if(previous)void api("/api/generation/projects/cancel","POST",{job_id:previous}).catch(()=>{});setPending(false);onBusyChange(false);}
 async function read(kind:"snapshot"|"directory"|"zip"){
  if(pending||disabled)return;const origin=scope,ticket=++generation.current;
  const valid=()=>mounted.current&&current.current===origin&&generation.current===ticket;
  setPending(true);onBusyChange(true);setError(false);
  try{
   let bundle:TemplateBundle;
   if(kind==="snapshot"){
    const text=await pickFile();if(!text||!valid())return;const value=JSON.parse(text);
    const source=value?.files?.find((file:ProjectFile)=>file.path==="moleapi-templates.json"&&file.encoding==="utf8");
    bundle=parseTemplateBundle(source?JSON.parse(source.content):value);
   }else{
    const source=await pickProjectSource(kind,valid);if(!source||!valid())return;
    const jobId=id();job.current=jobId;
    const imported=await api<{files:ProjectFile[]}>("/api/generation/projects/import","POST",{workspace_id:workspaceId,job_id:jobId,role:"working",source});
    if(!valid())return;
    const saved=imported.files.find(file=>file.path==="moleapi-templates.json");
    const decoder=new TextDecoder("utf8",{fatal:true});
    bundle=saved?parseTemplateBundle(JSON.parse(decoder.decode(decodeProjectBytes(saved.encoding,saved.content)))):parseTemplateBundle({format:"moleapi-codegen-templates-v1",files:imported.files.map(file=>({path:file.path,content:decoder.decode(decodeProjectBytes(file.encoding,file.content))}))});
   }
   if(valid()){setPath(bundle.files[0].path);job.current=null;setPending(false);onBusyChange(false);onChange(bundle);}
  }catch{if(valid())setError(true);}
  finally{if(valid()){job.current=null;setPending(false);onBusyChange(false);}}
 }
 return <details><summary>{t("自定义 Mustache 模板")}{value?` (${value.files.length})`:""}</summary><Flex direction="column" gap="2" mt="2">
  <Text size="1" color="gray">{t("显式覆盖上游 .mustache 文件；目录或 ZIP 内路径应相对模板根目录。最多 128 个文件，每个 64 KiB，合计 512 KiB。")}</Text>
  <Flex gap="2" wrap="wrap"><Button variant="soft" disabled={disabled||pending} onClick={()=>void read("snapshot")}>{t("读取模板 JSON / 生成快照")}</Button><Button variant="soft" disabled={disabled||pending} onClick={()=>void read("directory")}>{t("模板目录")}</Button><Button variant="soft" disabled={disabled||pending} onClick={()=>void read("zip")}>{t("模板 ZIP / 生成项目 ZIP")}</Button>{value&&<Button variant="soft" disabled={disabled||pending} onClick={()=>onChange(null)}>{t("恢复上游模板")}</Button>}{pending&&<Button variant="soft" onClick={stop}>{t("取消模板导入")}</Button>}</Flex>
  {pending&&<Text role="status" size="2">{t("正在读取模板…")}</Text>}
  {error&&<Callout.Root color="red"><Callout.Text role="alert">{t("模板导入失败，请检查文件格式、路径、UTF-8 编码和大小限制。")}</Callout.Text></Callout.Root>}
  {value&&selected&&<><Field label={t("模板文件")}><Choice label={t("模板文件")} value={selected.path} disabled={pending} options={value.files.map(file=>({value:file.path,label:file.path}))} onChange={setPath}/></Field><Editor value={selected.content} readOnly={pending||disabled} dark={dark} height="180px" onChange={content=>{try{onChange(parseTemplateBundle({...value,files:value.files.map(file=>file.path===selected.path?{...file,content}:file)}));setError(false);}catch{setError(true);}}}/></>}
 </Flex></details>;
}
