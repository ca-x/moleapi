import {useEffect,useRef,useState} from "react";
import stableStringify from "fast-json-stable-stringify";
import {Button,Callout,Flex,Text,TextArea,TextField} from "@radix-ui/themes";
import {api,native,token} from "../../shared/api";
import {t,useLanguage} from "../../shared/i18n";
import {liveError,LocalizedError} from "../../shared/i18n/errors";
import {Choice,Editor,Field} from "../../shared/ui";
import type {ExportResult} from "../../shared/types";
import {saveProjectFile} from "../generation/saveProjectFile";
import CiRequestPicker from "./CiRequestPicker";
import NotificationPicker from "../notifications/NotificationPicker";
import {useWorkbench} from "../workbench/context";
interface Preset {filename:string;mime:string;content:string;command:string[];prerequisites:string[]}
const lines=(source:string)=>source.split("\n").map(value=>value.trim()).filter(Boolean);
const fresh=()=>({provider:"github",source:native?"file":"remote",server:native?"":window.location.origin,path:"moleapi-workspace.json",execution:"collection",selected:"",environment:"",dataset:"",dataFile:"",dataFormat:"csv",iterations:"",requests:"",notifications:"silent",targets:"",reporter:"junit",language:"en",secretName:"MOLEAPI_TOKEN",variablesSecret:"",branches:""});
export default function CiPanel(){
 useLanguage();const state=useWorkbench(),workspace=state.draft,credential=token();
 const owner=JSON.stringify([state.accountId,workspace?.id,state.authenticated]);
 const boundary=useRef({owner,credential,epoch:0});if(boundary.current.owner!==owner||boundary.current.credential!==credential)boundary.current={owner,credential,epoch:boundary.current.epoch+1};
 const [form,setForm]=useState(fresh),[result,setResult]=useState<{identity:string;preset:Preset}|null>(null),[pending,setPending]=useState<string|null>(null),[error,setError]=useState<{identity:string;value:ReturnType<typeof liveError>}|null>(null);
 const identity=stableStringify([owner,boundary.current.epoch,workspace?.revision,state.dirty,form]);const latest=useRef({identity,credential});latest.current={identity,credential};
 const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
 useEffect(()=>{setForm({...fresh(),selected:workspace?.data.collections[0]?.id??""});setResult(null);setError(null);setPending(null);},[owner]);
 if(!workspace)return null;
 const current=(origin:string,auth=credential)=>mounted.current&&latest.current.identity===origin&&latest.current.credential===auth&&token()===auth;
 const change=(patch:Partial<typeof form>)=>setForm(value=>({...value,...patch}));const busy=pending===identity,preset=result?.identity===identity?result.preset:null;
 const config={provider:form.provider,source:form.source==="remote"?{kind:"remote",server:form.server,workspace:workspace.id}:{kind:"file",path:form.path,format:"moleapi"},collection:form.execution==="collection"?form.selected:null,scenario:form.execution==="scenario"?form.selected:null,environment:form.environment||null,dataset:form.dataFile?null:form.dataset||null,data_file:form.dataFile||null,data_format:form.dataFormat,iterations:form.iterations?Number(form.iterations):null,requests:form.execution==="collection"?lines(form.requests):[],notifications:form.source==="file"?{mode:"silent"}:form.notifications==="selected"?{mode:"selected",ids:lines(form.targets)}:{mode:form.notifications},reporter:form.reporter,language:form.language,secret_name:form.secretName,variables_secret:form.variablesSecret||null,branches:lines(form.branches)};
 async function generate(){if(busy||state.dirty||!workspace||!current(identity))return;const origin=identity;setPending(origin);setError(null);setResult(null);try{const value=await api<Preset>(`/api/workspaces/${workspace.id}/ci-preset`,"POST",{expected_revision:workspace.revision,config});if(current(origin))setResult({identity:origin,preset:value});}catch(error){if(current(origin))setError({identity:origin,value:liveError(error)});}finally{if(current(origin))setPending(null);}}
 async function download(){if(!preset)return;const origin=identity;try{await saveProjectFile(preset.filename,new TextEncoder().encode(preset.content),()=>current(origin));}catch(error){if(current(origin))setError({identity:origin,value:liveError(error)});}}
 async function copy(){if(!preset||!current(identity))return;const origin=identity;try{await navigator.clipboard.writeText(preset.content);}catch(error){if(current(origin))setError({identity:origin,value:liveError(error)});}}
 async function snapshot(){if(!workspace||state.dirty||!current(identity))return;const origin=identity;try{const file=await api<ExportResult>(`/api/workspaces/${workspace.id}/export`,"POST",{format:"moleapi",include_secrets:false});const value=JSON.parse(file.content);if(value.id!==workspace.id||value.revision!==workspace.revision)throw new LocalizedError("工作区已更新，请刷新后重新生成 CI 配置。");if(current(origin))await saveProjectFile(form.path.split(/[\\/]/).pop()||file.filename,new TextEncoder().encode(file.content),()=>current(origin));}catch(error){if(current(origin))setError({identity:origin,value:liveError(error)});}}
 return <details><summary>{t("Git 与 CI 测试配置")}</summary><Flex direction="column" gap="3" mt="3">
 <Text size="2" color="gray">{t("生成 Git 提交触发的 API 测试配置；生成时不会运行请求，也不会修改 CI 平台。")}</Text>
 {error?.identity===identity&&<Callout.Root color="red" role="alert"><Callout.Text>{error.value}</Callout.Text></Callout.Root>}
 <Flex gap="3" wrap="wrap"><Choice label={t("CI 平台")} value={form.provider} options={[{value:"github",label:"GitHub Actions"},{value:"gitlab",label:"GitLab CI"},{value:"jenkins",label:"Jenkins"}]} onChange={provider=>change({provider})}/><Choice label={t("运行来源")} value={form.source} options={[{value:"remote",label:t("服务端保存的工作区")},{value:"file",label:t("仓库中的集合文件")}]} onChange={source=>change({source})}/></Flex>
 {form.source==="remote"?<><Field label={t("服务端地址")}><TextField.Root value={form.server} onChange={event=>change({server:event.target.value})}/></Field><Field label={t("CI 密钥引用名称")}><TextField.Root value={form.secretName} maxLength={64} onChange={event=>change({secretName:event.target.value})}/></Field><Text size="1" color="gray">{t("在 CI 平台配置该名称的访问令牌密钥；配置文件不包含令牌原文。")}</Text></>:<><Field label={t("集合文件在仓库中的路径")}><TextField.Root value={form.path} onChange={event=>change({path:event.target.value})}/></Field><Button variant="soft" disabled={state.dirty} onClick={()=>void snapshot()}>{t("导出 CI 集合文件")}</Button><Text size="1" color="gray">{t("导出使用原生格式并隐藏私密值；请在 CI 运行输入中补充所需值。文件运行不发送通知。")}</Text></>}
 <Field label={t("运行变量密钥引用（可选）")}><TextField.Root value={form.variablesSecret} maxLength={64} onChange={event=>change({variablesSecret:event.target.value})}/></Field><Text size="1" color="gray">{t("该 CI 密钥保存 JSON 变量覆盖，运行时读取；与服务端登录令牌分开，不写入集合或配置文件。")}</Text>
 <Flex gap="3" wrap="wrap"><Choice label={t("执行对象类型")} value={form.execution} options={[{value:"collection",label:t("集合")},{value:"scenario",label:t("场景")}]} onChange={execution=>change({execution,selected:execution==="collection"?workspace.data.collections[0]?.id??"":workspace.data.scenarios?.[0]?.id??""})}/><Choice label={t("执行对象")} value={form.selected} options={[{value:"",label:t("请选择")},...(form.execution==="collection"?workspace.data.collections:workspace.data.scenarios??[]).map(value=>({value:value.id,label:value.name}))]} onChange={selected=>change({selected,requests:""})}/><Choice label={t("环境")} value={form.environment} options={[{value:"",label:t("工作区当前环境")},...workspace.data.environments.map(value=>({value:value.id,label:value.name}))]} onChange={environment=>change({environment})}/></Flex>
 <Choice label={t("保存的数据集")} value={form.dataset} options={[{value:"",label:t("无数据集")},...(workspace.data.datasets??[]).map(value=>({value:value.id,label:value.name}))]} onChange={dataset=>change({dataset,dataFile:""})}/><Field label={t("仓库中的 CSV/JSON 数据文件（可选）")}><TextField.Root value={form.dataFile} onChange={event=>change({dataFile:event.target.value,dataset:""})}/></Field>
 {form.dataFile&&<Choice label={t("数据格式")} value={form.dataFormat} options={[{value:"csv",label:"CSV"},{value:"json",label:"JSON"}]} onChange={dataFormat=>change({dataFormat})}/>}
 <Field label={t("迭代次数（1–100，可空）")}><TextField.Root type="number" min={1} max={100} value={form.iterations} onChange={event=>change({iterations:event.target.value})}/></Field>
 {form.execution==="collection"&&<CiRequestPicker collections={workspace.data.collections} root={form.selected} value={lines(form.requests)} onChange={requests=>change({requests:requests.join("\n")})}/>}
 {form.source==="remote"&&<><Choice label={t("通知模式")} value={form.notifications} options={[{value:"silent",label:t("不发送通知")},{value:"defaults",label:t("使用场景默认通知")},{value:"selected",label:t("指定通知对象")}]} onChange={notifications=>change({notifications})}/>{form.notifications==="selected"&&<NotificationPicker label={t("指定通知对象")} value={lines(form.targets)} onChange={targets=>change({targets:targets.join("\n")})} disabled={busy}/>}</>}
 <Flex gap="3" wrap="wrap"><Choice label={t("报告格式")} value={form.reporter} options={[{value:"junit",label:"JUnit"},{value:"json",label:"JSON"},{value:"html",label:"HTML"},{value:"csv",label:"CSV"}]} onChange={reporter=>change({reporter})}/><Choice label={t("报告语言")} value={form.language} options={[{value:"en",label:"English"},{value:"zh-CN",label:t("简体中文")}]} onChange={language=>change({language})}/></Flex>
 <Field label={t("触发分支（精确名称，每行一个，可空）")}><TextArea value={form.branches} onChange={event=>change({branches:event.target.value})}/></Field>
 <Text size="1" color="gray">{t("CI 执行器需已安装 moleapi-cli；报告归档在测试失败时也会执行。Jenkins 需配置来自 SCM 的流水线任务。")}</Text>
 {state.dirty&&<Text role="status">{t("请先保存工作区，再生成 CI 配置。")}</Text>}
 <Button loading={busy} disabled={state.dirty||!form.selected||busy} onClick={()=>void generate()}>{t("生成 CI 配置")}</Button>
 {preset&&<><Editor value={preset.content} readOnly dark={state.dark} height="280px" label={t("CI 配置预览")}/><Flex gap="3"><Button variant="soft" onClick={()=>void copy()}>{t("复制 CI 配置")}</Button><Button variant="soft" onClick={()=>void download()}>{t("下载 CI 配置")}</Button></Flex></>}
 </Flex></details>;
}
