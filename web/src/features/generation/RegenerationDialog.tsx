import {useEffect,useRef,useState} from "react";
import {z} from "zod";
import {Button,Callout,Dialog,Flex,Grid,Text} from "@radix-ui/themes";
import {api,pickFile} from "../../shared/api";
import {t,useLanguage} from "../../shared/i18n";
import {Choice,Editor,Field} from "../../shared/ui";
import {useWorkbench} from "../workbench/context";
import {id} from "../../shared/model";
import type {ProjectArtifact,ProjectFile} from "./projectTypes";
import {decodeProjectBytes,saveProjectFile} from "./saveProjectFile";
import {pickProjectSource} from "./projectInputs";
import type {WorkingFile} from "./projectInputs";
const file=z.object({path:z.string().max(1024),encoding:z.enum(["utf8","base64"]),content:z.string().max(6*1024*1024),bytes:z.number().int().nonnegative(),sha256:z.string(),executable:z.boolean().default(false)});
const snapshot=z.object({files:z.array(file).max(2048)});
interface MergedFile {path:string;status:string;encoding:string;content:string|null;executable:boolean;patch:string|null}
interface MergeResult {files:MergedFile[];conflicts:number;archive_base64:string|null}
const statuses:Record<string,()=>string>={unchanged:()=>t("未变化"),updated:()=>t("更新生成内容"),preserved:()=>t("保留编辑"),authored:()=>t("用户文件"),added:()=>t("新增文件"),merged:()=>t("已合并"),conflict:()=>t("合并冲突"),resolution_pending:()=>t("已选择，等待合并"),resolved:()=>t("已解决冲突"),deleted:()=>t("已删除")};
export default function RegenerationDialog({next,open,onOpenChange}:{next:ProjectArtifact;open:boolean;onOpenChange:(value:boolean)=>void}){
 useLanguage();const state=useWorkbench();
 const [previous,setPrevious]=useState<ProjectFile[]|null>(null),[working,setWorking]=useState<WorkingFile[]|null>(null),[resolutions,setResolutions]=useState<Record<string,string>>({}),[result,setResult]=useState<MergeResult|null>(null),[path,setPath]=useState("");
 const [busy,setBusy]=useState<"import"|"merge"|null>(null),[error,setError]=useState<"import"|"merge"|"save"|null>(null);
 const job=useRef<string|null>(null),generation=useRef(0),mounted=useRef(true);
 const scope=JSON.stringify([state.accountId,state.draft?.id,open,next.source_sha256,next.target]);
 const current=useRef({scope,next});current.current={scope,next};
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;generation.current++;const pending=job.current;if(pending)void api("/api/generation/projects/cancel","POST",{job_id:pending}).catch(()=>{});};},[]);
 useEffect(()=>{setPrevious(null);setWorking(null);setResult(null);setError(null);setBusy(null);setResolutions({});generation.current++;return()=>{generation.current++;const pending=job.current;job.current=null;if(pending)void api("/api/generation/projects/cancel","POST",{job_id:pending}).catch(()=>{});};},[scope,next]);
 async function read(which:"previous"|"working",kind:"snapshot"|"zip"|"directory"){
  if(busy||!state.draft)return;
  const ticket=++generation.current,origin=scope,source=next;
  const valid=()=>mounted.current&&current.current.scope===origin&&current.current.next===source&&generation.current===ticket;
  setError(null);setBusy("import");
  try {
   let files:ProjectFile[]|WorkingFile[];
   if(kind==="snapshot"){
    const input=await pickFile();if(!input||!valid())return;
    files=snapshot.parse(JSON.parse(input)).files;
   } else {
    const input=await pickProjectSource(kind,valid,new Map(previous?.map(file=>[file.path,file.executable])));if(!input||!valid())return;
    const jobId=id();job.current=jobId;
    const imported=await api<{files:ProjectFile[]}>("/api/generation/projects/import","POST",{workspace_id:state.draft.id,job_id:jobId,role:which,source:input});
    if(!valid())return;files=imported.files;
   }
   if(!valid())return;
   if(which==="previous"){setPrevious(files as ProjectFile[]);setWorking(files);}else setWorking(files);
   setResult(null);setResolutions({});
  } catch {if(valid())setError("import");}
  finally {if(valid()){job.current=null;setBusy(null);}}
 }
 async function merge(){
  if(busy||!previous||!working||!state.draft)return;
  const origin=scope,source=next,ticket=++generation.current,jobId=id();
  const valid=()=>mounted.current&&current.current.scope===origin&&current.current.next===source&&generation.current===ticket;
  job.current=jobId;setBusy("merge");setError(null);
  try{
   const value=await api<MergeResult>("/api/generation/projects/regenerate","POST",{workspace_id:state.draft.id,job_id:jobId,previous,working:working.map(({path,encoding,content,executable})=>({path,encoding,content,executable})),next:next.files,resolutions});
   if(valid()){setResult(value);setPath(value.files.some(file=>file.path===path)?path:value.files[0]?.path??"");}
  }catch{if(valid())setError("merge");}finally{if(valid()){job.current=null;setBusy(null);}}
 }
 function stop(){generation.current++;const pending=job.current;job.current=null;setBusy(null);if(pending)void api("/api/generation/projects/cancel","POST",{job_id:pending}).catch(()=>{});}
 async function download(){
  if(!result?.archive_base64)return;
  const origin=scope,source=next,ticket=generation.current;
  const valid=()=>mounted.current&&current.current.scope===origin&&current.current.next===source&&generation.current===ticket;
  try{await saveProjectFile("regenerated-project.zip",decodeProjectBytes("base64",result.archive_base64),valid);}catch{if(valid())setError("save");}
 }
 const selected=result?.files.find(file=>file.path===path);
 function pickResolution(choice:string){
  setResolutions({...resolutions,[path]:choice});
  setResult(previous=>previous?{...previous,archive_base64:null,files:previous.files.map(file=>file.path===path?{...file,status:"resolution_pending"}:file)}:null);
 }
 return <Dialog.Root open={open} onOpenChange={onOpenChange}><Dialog.Content className="project-regeneration-dialog" maxWidth="1000px"><Dialog.Title>{t("再生成差异与合并")}</Dialog.Title><Dialog.Description>{t("比较旧生成项目、当前编辑文件与新生成结果。支持快照 JSON、ZIP 和当前项目目录；冲突需要明确选择。")}</Dialog.Description>
 <Flex direction="column" gap="3" mt="3" aria-busy={!!busy}>
 <Grid columns={{initial:"1",sm:"2"}} gap="4">
  <Flex direction="column" gap="2"><Text weight="medium">{t("旧生成项目")}</Text><Text size="1" color="gray">{t("使用未修改的生成快照或原始项目 ZIP，校验和必须一致。")}</Text>
   <Flex gap="2" wrap="wrap"><Button aria-label={t("读取旧生成快照 JSON")} highContrast variant="soft" disabled={!!busy} onClick={()=>void read("previous","snapshot")}>{t("快照 JSON")}</Button><Button aria-label={t("读取旧生成项目 ZIP")} highContrast variant="soft" disabled={!!busy} onClick={()=>void read("previous","zip")}>{t("项目 ZIP")}</Button></Flex>
  </Flex>
  <Flex direction="column" gap="2"><Text weight="medium">{t("当前编辑项目")}</Text><Text size="1" color="gray">{t("读取整个项目：新增文件会保留，缺失文件视为删除。")}</Text>
   <Flex gap="2" wrap="wrap"><Button aria-label={t("读取当前项目目录")} highContrast variant="soft" disabled={!!busy||!previous} onClick={()=>void read("working","directory")}>{t("项目目录")}</Button><Button aria-label={t("读取当前项目 ZIP")} highContrast variant="soft" disabled={!!busy||!previous} onClick={()=>void read("working","zip")}>{t("项目 ZIP")}</Button><Button aria-label={t("读取当前编辑快照 JSON")} highContrast variant="soft" disabled={!!busy||!previous} onClick={()=>void read("working","snapshot")}>{t("快照 JSON")}</Button></Flex>
  </Flex>
 </Grid>
 <Text size="1" color="gray">{t("目录导入跳过 .git、.svn、.hg、node_modules、target、.venv 和 __pycache__；ZIP 导入保留其中的文件。导入和合并不会覆盖原文件。")}</Text>
 {previous&&working&&<Text size="2" role="status">{t("旧生成 {{previous}} 个文件，当前编辑 {{working}} 个文件，新生成 {{next}} 个文件。",{previous:previous.length,working:working.length,next:next.files.length})}</Text>}
 <Flex gap="2" wrap="wrap"><Button disabled={!!busy||!previous||!working} onClick={()=>void merge()}>{t("比较并合并")}</Button>{busy&&<Button highContrast variant="soft" onClick={stop}>{t("取消")}</Button>}<Button highContrast variant="soft" disabled={!result?.archive_base64||!!busy} onClick={()=>void download()}>{t("下载合并项目 ZIP")}</Button></Flex>
 {busy&&<Text role="status">{busy==="import"?t("正在读取项目文件…"):t("正在计算再生成差异…")}</Text>}
 {error&&<Callout.Root color="red"><Callout.Text role="alert">{error==="import"?t("无法导入项目，请检查 ZIP 或快照、原始校验和、文件类型与大小限制。"):error==="save"?t("无法保存生成文件。"):t("无法比较项目，请检查快照 JSON、文件校验和及大小限制。")}</Callout.Text></Callout.Root>}
 {result&&<><Text>{t("冲突数量：{{count}}",{count:result.conflicts})}</Text><Field label={t("比较文件")}><Choice label={t("比较文件")} value={path} onChange={setPath} options={result.files.map(file=>({value:file.path,label:`${file.path} · ${statuses[file.status]?.()??file.status}`}))}/></Field>
 {(selected?.status==="conflict"||selected?.status==="resolution_pending")&&<Flex gap="2" wrap="wrap"><Button highContrast variant="soft" disabled={!!busy} onClick={()=>pickResolution("working")}>{t("保留当前编辑版本")}</Button><Button highContrast variant="soft" disabled={!!busy} onClick={()=>pickResolution("generated")}>{t("使用新生成版本")}</Button><Button highContrast variant="soft" color="red" disabled={!!busy} onClick={()=>pickResolution("delete")}>{t("确认删除此文件")}</Button></Flex>}
 {selected?.status==="resolution_pending"&&<Text size="1" role="status">{t("选择已记录，可继续处理其他冲突；点击比较并合并后应用全部选择。")}</Text>}
 {selected&&<Editor value={selected.patch??selected.content??t("此文件已删除。")} readOnly dark={state.dark} height="40vh"/>}</>}
 <Flex justify="end"><Dialog.Close><Button highContrast variant="soft">{t("关闭")}</Button></Dialog.Close></Flex></Flex></Dialog.Content></Dialog.Root>;
}
