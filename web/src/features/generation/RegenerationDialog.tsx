import {useEffect,useRef,useState} from "react";
import {z} from "zod";
import {Button,Callout,Dialog,Flex,Text} from "@radix-ui/themes";
import {api,pickFile} from "../../shared/api";
import {t,useLanguage} from "../../shared/i18n";
import {Choice,Editor,Field} from "../../shared/ui";
import {useWorkbench} from "../workbench/context";
import {id} from "../../shared/model";
import type {ProjectArtifact,ProjectFile} from "./projectTypes";
import {decodeProjectBytes,saveProjectFile} from "./saveProjectFile";
const file=z.object({path:z.string().max(1024),encoding:z.enum(["utf8","base64"]),content:z.string().max(6*1024*1024),bytes:z.number().int().nonnegative(),sha256:z.string(),executable:z.boolean().default(false)});
const snapshot=z.object({files:z.array(file).max(2048)});
interface MergedFile {path:string;status:string;encoding:string;content:string|null;executable:boolean;patch:string|null}
interface MergeResult {files:MergedFile[];conflicts:number;archive_base64:string|null}
const statuses:Record<string,()=>string>={unchanged:()=>t("未变化"),updated:()=>t("更新生成内容"),preserved:()=>t("保留编辑"),authored:()=>t("用户文件"),added:()=>t("新增文件"),merged:()=>t("已合并"),conflict:()=>t("合并冲突"),resolved:()=>t("已解决冲突"),deleted:()=>t("已删除")};
export default function RegenerationDialog({next,open,onOpenChange}:{next:ProjectArtifact;open:boolean;onOpenChange:(value:boolean)=>void}){
 useLanguage();const state=useWorkbench();const [previous,setPrevious]=useState<ProjectFile[]|null>(null),[working,setWorking]=useState<ProjectFile[]|null>(null),[resolutions,setResolutions]=useState<Record<string,string>>({}),[result,setResult]=useState<MergeResult|null>(null),[path,setPath]=useState(""),[busy,setBusy]=useState(false),[error,setError]=useState(false);
 const job=useRef<string|null>(null),generation=useRef(0),mounted=useRef(true);const scope=JSON.stringify([state.accountId,state.draft?.id,open,next.source_sha256,next.target]);const current=useRef(scope);current.current=scope;
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;generation.current++;const pending=job.current;if(pending)void api("/api/generation/projects/cancel","POST",{job_id:pending}).catch(()=>{});};},[]);
 useEffect(()=>{setPrevious(null);setWorking(null);setResult(null);setError(false);setBusy(false);setResolutions({});generation.current++;return()=>{const pending=job.current;job.current=null;if(pending)void api("/api/generation/projects/cancel","POST",{job_id:pending}).catch(()=>{});};},[scope]);
 async function read(which:"previous"|"working"){
   const origin=scope,ticket=++generation.current;setError(false);
   try {const input=await pickFile();if(!input||!mounted.current||current.current!==origin||generation.current!==ticket)return;const files=snapshot.parse(JSON.parse(input)).files;
    if(which==="previous"){setPrevious(files);setWorking(files);}else setWorking(files);setResult(null);setResolutions({});
   }catch{if(mounted.current&&current.current===origin&&generation.current===ticket)setError(true);}
 }
 async function merge(){if(busy||!previous||!working||!state.draft)return;const origin=scope,ticket=++generation.current,jobId=id();job.current=jobId;setBusy(true);setError(false);
  try{const value=await api<MergeResult>("/api/generation/projects/regenerate","POST",{workspace_id:state.draft.id,job_id:jobId,previous,working:working.map(({path,encoding,content,executable})=>({path,encoding,content,executable})),next:next.files,resolutions});if(mounted.current&&current.current===origin&&generation.current===ticket){setResult(value);setPath(value.files[0]?.path??"");}}
  catch{if(mounted.current&&current.current===origin&&generation.current===ticket)setError(true);}finally{if(job.current===jobId){job.current=null;if(mounted.current)setBusy(false);}}
 }
 function stop(){generation.current++;const pending=job.current;job.current=null;setBusy(false);if(pending)void api("/api/generation/projects/cancel","POST",{job_id:pending}).catch(()=>{});}
 async function download(){if(!result?.archive_base64)return;const origin=scope;await saveProjectFile("regenerated-project.zip",decodeProjectBytes("base64",result.archive_base64),()=>mounted.current&&current.current===origin);}
 const selected=result?.files.find(f=>f.path===path);const pickResolution=(choice:string)=>{setResolutions({...resolutions,[path]:choice});setResult(null);};
 return <Dialog.Root open={open} onOpenChange={onOpenChange}><Dialog.Content maxWidth="1000px"><Dialog.Title>{t("再生成差异与合并")}</Dialog.Title><Dialog.Description>{t("比较旧生成快照、当前编辑快照与新生成结果。只输出新归档，不覆盖原文件；冲突需要明确选择。")}</Dialog.Description>
 <Flex direction="column" gap="3" mt="3"><Flex gap="2" wrap="wrap"><Button variant="soft" disabled={busy} onClick={()=>void read("previous")}>{t("读取旧生成快照 JSON")}</Button><Button variant="soft" disabled={busy||!previous} onClick={()=>void read("working")}>{t("读取当前编辑快照 JSON")}</Button><Button disabled={busy||!previous||!working} onClick={()=>void merge()}>{t("比较并合并")}</Button>{busy&&<Button onClick={stop}>{t("停止生成")}</Button>}<Button variant="soft" disabled={!result?.archive_base64||busy} onClick={()=>void download()}>{t("下载合并项目 ZIP")}</Button></Flex>
 <Text size="1" color="gray">{t("从生成窗口保存快照；当前编辑快照可修改 content 或添加文件，删除条目代表删除文件。原始快照的内容和校验和必须保持一致。")}</Text>
 {busy&&<Text role="status">{t("正在计算再生成差异…")}</Text>}{error&&<Callout.Root color="red"><Callout.Text role="alert">{t("无法比较项目，请检查快照 JSON、文件校验和及大小限制。")}</Callout.Text></Callout.Root>}
 {result&&<><Text>{t("冲突数量：{{count}}",{count:result.conflicts})}</Text><Field label={t("比较文件")}><Choice label={t("比较文件")} value={path} onChange={setPath} options={result.files.map(f=>({value:f.path,label:`${f.path} · ${statuses[f.status]?.()??f.status}`}))}/></Field>{selected?.status==="conflict"&&<Flex gap="2" wrap="wrap"><Button variant="soft" onClick={()=>pickResolution("working")}>{t("保留当前编辑版本")}</Button><Button variant="soft" onClick={()=>pickResolution("generated")}>{t("使用新生成版本")}</Button><Button variant="soft" color="red" onClick={()=>pickResolution("delete")}>{t("确认删除此文件")}</Button></Flex>}
 {selected&&<Editor value={selected.patch??selected.content??t("此文件已删除。")} readOnly dark={state.dark} height="40vh"/>}</>}
 <Flex justify="end"><Dialog.Close><Button variant="soft">{t("关闭")}</Button></Dialog.Close></Flex></Flex></Dialog.Content></Dialog.Root>;
}
