import RegenerationDialog from "./RegenerationDialog";
import {useEffect,useRef,useState} from "react";
import stableStringify from "fast-json-stable-stringify";
import {Button,Callout,Checkbox,Dialog,Flex,Text,TextField,ScrollArea} from "@radix-ui/themes";
import {Code2,Copy,Download,Square} from "lucide-react";
import {api} from "../../shared/api";
import {Choice,Editor,Field} from "../../shared/ui";
import {t,useLanguage,message,translateCopy,type LocalizedCopy} from "../../shared/i18n";
import {useWorkbench} from "../workbench/context";
import {id} from "../../shared/model";
import type {ProjectArtifact,ProjectCatalog} from "./projectTypes";
import {decodeProjectBytes,saveProjectFile} from "./saveProjectFile";
const optionLabels:Record<string,()=>string>={packageName:()=>t("包名"),packageVersion:()=>t("包版本"),apiPackage:()=>t("API 包名"),modelPackage:()=>t("模型包名"),invokerPackage:()=>t("调用器包名"),artifactId:()=>t("项目标识"),artifactVersion:()=>t("项目版本"),library:()=>t("HTTP 库 / 运行时"),namespace:()=>t("命名空间"),npmName:()=>t("npm 包名"),npmVersion:()=>t("npm 包版本"),groupId:()=>t("组织标识"),interface:()=>t("Rust 调用风格")};
export default function ProjectGenerationDialog({open,specificationId,onOpenChange}:{open:boolean;specificationId:string;onOpenChange:(open:boolean)=>void}){
 useLanguage();const state=useWorkbench();const spec=state.draft?.data.specifications?.find(s=>s.id===specificationId&&["openapi","protobuf"].includes(s.kind));
 const [compare,setCompare]=useState(false);
 const [catalog,setCatalog]=useState<ProjectCatalog|null>(null),[kind,setKind]=useState(spec?.kind==="protobuf"?"protobuf":"client"),[target,setTarget]=useState(spec?.kind==="protobuf"?"rust-tonic":"rust-progenitor"),[options,setOptions]=useState<Record<string,string>>({}),[include,setInclude]=useState(false);
 const [result,setResult]=useState<{identity:string;artifact:ProjectArtifact}|null>(null),[pending,setPending]=useState<string|null>(null),[error,setError]=useState<LocalizedCopy|null>(null),[path,setPath]=useState("");
 const owner=JSON.stringify([state.authenticated,state.accountId,state.draft?.id]);const boundary=JSON.stringify([owner,open]);const epoch=useRef({boundary,count:0});if(epoch.current.boundary!==boundary){epoch.current={boundary,count:epoch.current.count+1};}
 const identity=stableStringify([epoch.current.count,owner,open,spec,target,options,include]);const latest=useRef({identity,state});latest.current={identity,state};const mounted=useRef(true);const job=useRef<string|null>(null);
 const selected=catalog?.targets.find(t=>t.id===target);const artifact=result?.identity===identity?result.artifact:null;const busy=!!pending;const file=artifact?.files.find(f=>f.path===path)??artifact?.files[0];
 const protocMissing=target.startsWith("protobuf-")&&!catalog?.protoc_available;
 const javaMissing=!target.startsWith("protobuf-")&&!["rust-progenitor","rust-progenitor-cli","rust-tonic"].includes(target)&&!catalog?.java_available;
 const native31=["rust-progenitor","rust-progenitor-cli"].includes(target)&&spec?.dialect?.startsWith("3.1");
 function stop(){const previous=job.current;job.current=null;if(previous)void api("/api/generation/projects/cancel","POST",{job_id:previous}).catch(()=>{});setPending(null);}
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;const previous=job.current;if(previous)void api("/api/generation/projects/cancel","POST",{job_id:previous}).catch(()=>{});};},[]);
 useEffect(()=>{setInclude(false);setResult(null);setPending(null);setError(null);setOptions({});return()=>{const previous=job.current;job.current=null;if(previous)void api("/api/generation/projects/cancel","POST",{job_id:previous}).catch(()=>{});};},[boundary,specificationId]);
 useEffect(()=>{if(catalog&&catalog.targets.find(t=>t.id===target)?.kind!==kind){const next=catalog.targets.find(t=>t.kind===kind);if(next)choose(next.id);}},[catalog,kind,target]);
 useEffect(()=>{if(!open||!state.authenticated)return;let valid=true;setCatalog(null);api<ProjectCatalog>("/api/generation/projects/catalog").then(c=>{if(valid)setCatalog(c);}).catch(()=>{if(valid)setError(message("无法读取项目生成器，请关闭后重试。"));});return()=>{valid=false;};},[boundary]);
 function choose(value:string){setTarget(value);setOptions({});setResult(null);setError(null);}
 async function generate(){
  if(busy||!spec||!state.draft||javaMissing||native31||protocMissing)return;const origin=identity,snapshot=state,workspaceId=state.draft.id,ticket=id();const current=()=>mounted.current&&latest.current.identity===origin&&job.current===ticket;
  job.current=ticket;setPending(origin);setError(null);setResult(null);
  try{if(snapshot.dirty&&!(await snapshot.save(true)))return;if(!current())return;
   const result=await api<ProjectArtifact>("/api/generation/projects","POST",{workspace_id:workspaceId,specification_id:specificationId,job_id:ticket,target,options:Object.fromEntries(Object.entries(options).filter(([,v])=>v!=="")),include_secrets:include});
   if(current()){setResult({identity:origin,artifact:result});setPath(result.files[0]?.path??"");}
  }catch{if(current())setError(message("项目生成失败，请检查规范、生成器选项和运行时依赖。"));}
  finally{if(job.current===ticket){job.current=null;if(mounted.current)setPending(null);}}
 }
 async function saveSnapshot(){if(!artifact)return;const origin=identity;await saveProjectFile(`${target}-snapshot.json`,new TextEncoder().encode(JSON.stringify(artifact,null,2)),()=>mounted.current&&latest.current.identity===origin);}
 async function download(all:boolean){if(!artifact)return;const origin=identity;const current=()=>mounted.current&&latest.current.identity===origin;
  try{if(all)await saveProjectFile(`${target}-project.zip`,decodeProjectBytes("base64",artifact.archive_base64),current);else if(file)await saveProjectFile(file.path.split("/").pop()!,decodeProjectBytes(file.encoding,file.content),current);}catch{if(current())setError(message("无法保存生成文件。"));}
 }
 async function copy(){if(file?.encoding!=="utf8")return;try{await navigator.clipboard.writeText(file.content);}catch{setError(message("剪贴板不可用"));}}
 return <Dialog.Root open={open} onOpenChange={onOpenChange}><Dialog.Content maxWidth="1100px"><Dialog.Title><Flex gap="2" align="center"><Code2 size={20}/>{t("生成 SDK / 服务端项目")}</Flex></Dialog.Title><Dialog.Description>{t("基于原始 OpenAPI 定义生成独立项目，不覆盖已有代码，也不执行或发布生成结果。")}</Dialog.Description>
 <Flex direction="column" gap="3" mt="3">
 <Text size="2">{spec?.name??t("OpenAPI 定义已不可用")}</Text>
 <Flex gap="3" wrap="wrap"><Field label={t("项目类型")}><Choice label={t("项目类型")} value={kind} disabled={busy} options={spec?.kind==="protobuf"?[{value:"protobuf",label:t("Protobuf / gRPC 客户端及服务端")}]:[{value:"client",label:t("客户端 SDK")},{value:"server",label:t("服务端框架")},{value:"cli",label:t("API 命令行工具")}]} onChange={kind=>{setKind(kind);const next=catalog?.targets.find(t=>t.kind===kind);if(next)choose(next.id);}}/></Field>
 <Field label={t("项目生成器")}><Choice label={t("项目生成器")} value={target} disabled={busy||!catalog} options={(catalog?.targets??[]).filter(t=>t.kind===kind).map(t=>({value:t.id,label:`${t.id}${t.upstream_stability!=="stable"?` (${t.upstream_stability})`:""}`}))} onChange={choose}/></Field></Flex>
 {protocMissing&&<Callout.Root><Callout.Text>{t("此目标需要部署方显式配置 protoc。原生 Rust gRPC 生成不需要外部编译器。")}</Callout.Text></Callout.Root>}
 {target.startsWith("protobuf-")&&!catalog?.grpc_plugins?.includes(target.slice("protobuf-".length))&&<Text size="1" color="gray">{t("当前生成 Protobuf 消息类型；gRPC 服务接口需要配置对应语言的插件。")}</Text>}
 {native31&&<Callout.Root><Callout.Text>{t("原生 Rust 生成器支持 OpenAPI 3.0；3.1 请使用其他生成器。")}</Callout.Text></Callout.Root>}
 {javaMissing&&<Callout.Root><Callout.Text>{t("此生成器需要部署方配置 Java 17+。原生 Rust SDK 不需要 Java。")}</Callout.Text></Callout.Root>}
 <Text size="1" color="gray">{t("上游生成器目录不代表每个目标都经过编译验证；请验证生成项目后再使用。")}</Text>
 <Flex gap="3" wrap="wrap">{Object.entries(selected?.options??(target==="rust-progenitor"?{packageName:"moleapi_sdk",packageVersion:"0.1.0",interface:"positional"}:{})).map(([name,value])=><Field key={name} label={optionLabels[name]?.()??name}><TextField.Root disabled={busy} maxLength={128} autoComplete="off" value={options[name]??""} placeholder={String(value??"")} onChange={e=>setOptions({...options,[name]:e.target.value})}/></Field>)}</Flex>
 <label className="checkbox-label"><Checkbox checked={include} disabled={busy} onCheckedChange={v=>setInclude(v===true)}/>{t("包含敏感信息（生成文件可能包含凭据）")}</label>
 <Flex gap="2"><Button disabled={busy||!catalog||!spec||javaMissing||native31||protocMissing} onClick={()=>void generate()}>{t("生成项目")}</Button>{busy&&<Button highContrast variant="soft" onClick={stop}><Square size={15}/>{t("停止生成")}</Button>}<Button highContrast variant="soft" disabled={!artifact} onClick={()=>void download(true)}><Download size={15}/>{t("下载项目 ZIP")}</Button></Flex>
 {busy&&<Text role="status" size="2">{t("正在生成项目…")}</Text>}{error&&<Callout.Root color="red"><Callout.Text role="alert">{translateCopy(error)}</Callout.Text></Callout.Root>}
 {artifact&&<><Text size="1" color="gray">{artifact.engine} · {artifact.files.length} {t("个文件")} · SHA256 {artifact.source_sha256.slice(0,12)}</Text><div className="project-artifact-grid"><ScrollArea className="project-artifact-files"><Flex direction="column" gap="1">{artifact.files.map(f=><Button highContrast key={f.path} variant={file?.path===f.path?"soft":"ghost"} onClick={()=>setPath(f.path)} style={{justifyContent:"flex-start",whiteSpace:"normal",height:"auto",textAlign:"left",overflowWrap:"anywhere"}}>{f.path}</Button>)}</Flex></ScrollArea><Flex direction="column" gap="2" style={{minWidth:0,flex:1}}><Flex gap="2" wrap="wrap"><Text size="1" style={{overflowWrap:"anywhere"}}>{file?.path}</Text><Button variant="ghost" disabled={file?.encoding!=="utf8"} onClick={()=>void copy()}><Copy size={14}/>{t("复制")}</Button><Button variant="ghost" onClick={()=>void download(false)}>{t("下载文件")}</Button></Flex>{file?.encoding==="utf8"?<Editor value={file.content} readOnly dark={state.dark} height="40vh" jsonMode={file.path.endsWith(".json")}/>:<Text>{t("二进制文件可下载查看。")}</Text>}</Flex></div></>}
 <Flex gap="2" wrap="wrap"><Button highContrast variant="soft" disabled={!artifact||busy} onClick={()=>void saveSnapshot()}>{t("保存生成快照 JSON")}</Button><Button highContrast variant="soft" disabled={!artifact||busy} onClick={()=>setCompare(true)}>{t("再生成差异与合并")}</Button></Flex>
 <Flex justify="end"><Dialog.Close><Button highContrast variant="soft">{t("关闭")}</Button></Dialog.Close></Flex>
 {artifact&&compare&&<RegenerationDialog next={artifact} open={compare} onOpenChange={setCompare}/>}
 </Flex></Dialog.Content></Dialog.Root>;
}
