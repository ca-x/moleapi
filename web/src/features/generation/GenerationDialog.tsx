import type { ErrorCopy } from "../../shared/i18n/errors";
import { errorCopy } from "../../shared/i18n/errors";
import { t, useLanguage, liveTranslation, message, translateCopy } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import stableStringify from "fast-json-stable-stringify";
import { Button, Callout, Checkbox, Dialog, Flex, Text } from "@radix-ui/themes";
import { Copy, Download, Code2 } from "lucide-react";
import { toast } from "sonner";
import { api } from "../../shared/api";
import { saveProjectFile } from "./saveProjectFile";
import { Choice, Editor, Field } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";
import type { Snippet, SnippetCatalog } from "./types";
const extensions:Record<string,string>={c:"c",csharp:"cs",clojure:"clj",dart:"dart",fsharp:"fsx",go:"go",http:"http",java:"java",js:"js",julia:"jl",kotlin:"kt",node:"js",objc:"m",ocaml:"ml",php:"php",powershell:"ps1",python:"py",r:"r",ruby:"rb",rust:"rs",shell:"sh",swift:"swift"};
export default function GenerationDialog({open,onOpenChange}:{open:boolean;onOpenChange:(open:boolean)=>void}) {
  useLanguage();
  const state=useWorkbench();
  const [catalog,setCatalog]=useState<SnippetCatalog|null>(null),[target,setTarget]=useState("shell"),[client,setClient]=useState("curl"),[include,setInclude]=useState(false);
  const [result,setResult]=useState<{identity:string;snippet:Snippet}|null>(null),[error,setError]=useState<{identity:string;message:ErrorCopy}|null>(null),[pending,setPending]=useState<string|null>(null);
  const owner=JSON.stringify([state.authenticated,state.accountId,state.draft?.id,state.request?.id]);
  const identity=stableStringify([owner,state.request,target,client,include,open]);
  const latest=useRef({identity,state});latest.current={identity,state};
  const mounted=useRef(true);
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  useEffect(()=>{setInclude(false);setResult(null);setError(null);setPending(null);},[owner,open]);
  useEffect(()=>{
    if(!open||!state.authenticated)return;
    let current=true;
    api<SnippetCatalog>("/api/generation/snippets/catalog").then(value=>{if(current)setCatalog(value);}).catch(()=>{if(current)setError({identity:latest.current.identity,message:message("无法读取生成器列表，请关闭后重试。")});});
    return()=>{current=false;};
  },[open,owner]);
  const selected=catalog?.targets.find(value=>value.target===target),snippet=result?.identity===identity?result.snippet:null,busy=pending===identity;
  async function generate() {
    if(busy||!state.request||!state.draft)return;
    const origin=identity,snapshot=latest.current.state;
    const current=()=>mounted.current&&latest.current.identity===origin;
    setPending(origin);setError(null);setResult(null);
    try {
      if(snapshot.dirty&&!(await snapshot.save(true)))return;
      if(!current())return;
      const value=await api<Snippet>("/api/generation/snippets","POST",{workspace_id:snapshot.draft!.id,request_id:snapshot.request!.id,target,client,include_secrets:include});
      if(current())setResult({identity:origin,snippet:value});
    } catch(error) {if(current())setError({identity:origin,message:errorCopy(error)});}
    finally {if(current())setPending(null);}
  }
  async function copy() {if(!snippet)return;try{await navigator.clipboard.writeText(snippet.code);toast.success(liveTranslation("已复制请求示例"));}catch{toast.error(liveTranslation("剪贴板不可用"));}}
  async function download() {if(!snippet)return;const origin=identity;try{await saveProjectFile(`request-${snippet.target}-${snippet.client}.${extensions[snippet.target]??"txt"}`,new TextEncoder().encode(snippet.code),()=>mounted.current&&latest.current.identity===origin);}catch{if(mounted.current&&latest.current.identity===origin)toast.error(liveTranslation("无法保存请求示例"));}}
  return <Dialog.Root open={open} onOpenChange={onOpenChange}><Dialog.Content maxWidth="min(920px, calc(100vw - 32px))" className="generation-dialog">
    <Dialog.Title>{t("生成请求代码")}</Dialog.Title><Dialog.Description>{t("将当前保存的 HTTP 请求转换为所选语言的请求代码，支持预览、复制和下载。默认隐藏凭据。")}</Dialog.Description>
    <Flex gap="3" wrap="wrap" align="end" my="4">
      <Field label={t("语言")}><Choice label={t("代码语言")} value={target} disabled={!catalog||busy} options={catalog?.targets.map(value=>({value:value.target,label:value.title}))||[]} onChange={value=>{setTarget(value);setClient(catalog?.targets.find(item=>item.target===value)?.clients[0]?.client||"");}}/></Field>
      <Field label={t("HTTP 库")}><Choice label={t("代码 HTTP 库")} value={client} disabled={!selected||busy} options={selected?.clients.map(value=>({value:value.client,label:value.title}))||[]} onChange={setClient}/></Field>
      <Button disabled={!catalog||busy} loading={busy} onClick={()=>void generate()}><Code2 size={15}/>{t("生成")}</Button>
    </Flex>
    <Text as="label" size="2"><Flex gap="2" align="center"><Checkbox checked={include} disabled={busy} onCheckedChange={value=>setInclude(value===true)}/>{t("包含保存的凭据（复制和下载会包含私密值）")}</Flex></Text>
    {error?.identity===identity&&<Callout.Root color="red" role="alert" my="3"><Callout.Text>{translateCopy(error.message)}</Callout.Text></Callout.Root>}
    <Text as="p" size="1" color="gray">{catalog?.engine||t("正在读取生成器…")} {t("· 使用上游生成器；各语言的编译与运行验证进度见覆盖清单。")}</Text>
    {busy&&<Text as="p" role="status">{t("正在生成请求示例…")}</Text>}
    {snippet&&<>{snippet.target==="csharp"&&<Text as="p" size="2">{snippet.client==="restsharp"?t("RestSharp 示例需要对应 NuGet 包和 RestSharp 命名空间；JSON 原始字符串需要 C# 11 或更新版本。放入异步方法或支持顶层 await 的项目中使用。"):t("HttpClient 示例需要 System.Net.Http 和 System.Net.Http.Headers 命名空间；JSON 原始字符串需要 C# 11 或更新版本。放入异步方法或支持顶层 await 的项目中使用。")}</Text>}<Editor value={snippet.code} dark={state.dark} readOnly height="360px" label={t("生成的请求代码")}/>{snippet.warnings.map((warning,index)=><Text as="p" size="1" color="gray" key={index}>{t(warning)}</Text>)}</>}
    {!snippet&&!busy&&<Text as="p" color="gray">{t("选择语言和 HTTP 库后生成。默认屏蔽凭据，不执行请求和脚本。")}</Text>}
    <Flex justify="between" gap="3" mt="4" wrap="wrap"><Flex gap="3"><Button variant="soft" color="gray" disabled={!snippet} onClick={()=>void copy()}><Copy size={15}/>{t("复制代码")}</Button><Button variant="soft" color="gray" disabled={!snippet} onClick={()=>void download()}><Download size={15}/>{t("下载代码")}</Button></Flex><Dialog.Close><Button variant="soft" color="gray">{t("关闭")}</Button></Dialog.Close></Flex>
  </Dialog.Content></Dialog.Root>;
}
