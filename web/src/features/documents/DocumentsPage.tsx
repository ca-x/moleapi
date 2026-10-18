import {useEffect,useMemo,useRef,useState} from "react";
import {Button,Card,Checkbox,Flex,Heading,Tabs,Text,TextField} from "@radix-ui/themes";
import {FilePlus2,Trash2} from "lucide-react";
import {t,useLanguage,message} from "../../shared/i18n";
import {id} from "../../shared/model";
import {Choice,Editor,Field} from "../../shared/ui";
import type {MarkdownDocument} from "../../shared/types";
import {useWorkbench} from "../workbench/context";
import {validateMarkdownDocuments} from "./validation";
import MarkdownPreview from "./MarkdownPreview";
import ApiReference from "./ApiReference";
import {searchWorkspace} from "./search";
import type {SearchResult} from "./search";

export default function DocumentsPage() {
  useLanguage();
  const state=useWorkbench();
  const {draft,updateData,save,setGuard,dirty,saving,dark,accountId}=state;
  const [selectedId,setSelectedId]=useState("");
  const [mode,setMode]=useState("preview");
  const [section,setSection]=useState("markdown");
  const [query,setQuery]=useState("");
  const documentHeadingRef=useRef<HTMLHeadingElement>(null);
  const focusDocumentRef=useRef(false);
  useEffect(()=>{setSelectedId("");setMode("preview");setSection("markdown");setQuery("");},[accountId,draft?.id]);
  useEffect(()=>{
    if(focusDocumentRef.current&&section==="markdown"){
      documentHeadingRef.current?.focus();
      focusDocumentRef.current=false;
    }
  },[selectedId,section,query]);
  const search=useMemo(()=>draft?searchWorkspace(draft.data,query):{results:[],total:0,truncated:false},[draft?.data,query]);
  if(!draft)return null;
  const documents=draft.data.documents??[];
  const requestIds=new Set(draft.data.collections.flatMap(collection=>collection.requests.map(request=>request.id)));
  const openRequest=(requestId:string)=>{
    const latest=state.stateRef.current.draft;
    if(state.accountRef.current!==accountId||latest?.id!==draft.id||!latest.data.collections.some(collection=>collection.requests.some(request=>request.id===requestId)))return false;
    state.setRequestId(requestId);
    state.setView("requests");
    state.setSidebar(false);
    return true;
  };
  const openSearchResult=(result:SearchResult)=>{
    const latest=state.stateRef.current.draft;
    if(state.accountRef.current!==accountId||latest?.id!==draft.id)return;
    if(result.kind==="request"){
      if(openRequest(result.id))setQuery("");
      return;
    }
    if(!latest.data.documents?.some(document=>document.id===result.id))return;
    focusDocumentRef.current=true;
    setSelectedId(result.id);setSection("markdown");setMode("preview");setQuery("");
  };
  const current=documents.find(document=>document.id===selectedId)??documents[0];
  const selected=current?.id;
  const updateDocument=(patch:Partial<MarkdownDocument>)=>{
    if(!selected)return;
    updateData(data=>({...data,documents:(data.documents??[]).map(document=>document.id===selected?{...document,...patch}:document)}));
  };
  const create=()=>{
    if(documents.length>=256)return;
    const next:MarkdownDocument={id:id(),name:t("新建文档"),collection_id:null,source:"",secret:true};
    updateData(data=>({...data,documents:[...(data.documents??[]),next]}));
    setSelectedId(next.id);setMode("edit");
  };
  const remove=()=>{
    if(!current)return;
    setGuard({title:message("删除文档"),description:message("删除「{{value0}}」？保存工作区后生效。",{value0:current.name}),action:()=>{
      updateData(data=>({...data,documents:(data.documents??[]).filter(document=>document.id!==current.id)}));
      setSelectedId("");
    }});
  };
  const {invalidTitleDocument,invalidSourceDocument,invalidTotalSource}=validateMarkdownDocuments(documents);
  return <div className="page-panel documents-page">
    <div className="page-heading">
      <div><Heading size="5">{t("工作区文档")}</Heading><Text size="2" color="gray">{t("为工作区编写说明，并在保存前预览。")}</Text></div>
      <Flex gap="2" wrap="wrap">{section==="markdown"&&<Button variant="soft" color="gray" onClick={create} disabled={documents.length>=256}><FilePlus2 size={16}/>{t("新建文档")}</Button>}<Button variant={dirty?"solid":"soft"} color={dirty?undefined:"gray"} onClick={()=>void save()} disabled={!dirty||saving||!!invalidTitleDocument||!!invalidSourceDocument||invalidTotalSource} loading={saving}>{t("保存工作区")}</Button></Flex>
    </div>
    <div className="documents-search">
      <TextField.Root aria-label={t("搜索文档与接口")} placeholder={t("搜索文档与接口")} value={query} maxLength={128} onChange={event=>setQuery(event.target.value)}/>
      <Text size="1" color="gray">{t("仅搜索当前工作区；私密文档正文可能出现在结果中。")}</Text>
    </div>
    {query.trim()&&<Card className="documents-search-results">
      <Flex justify="between" align="center" gap="2" wrap="wrap"><Text role="status" size="2">{search.total===1?t("找到 1 项结果。"):t("找到 {{value0}} 项结果。",{value0:search.total})}</Text><Button size="1" variant="ghost" color="gray" onClick={()=>setQuery("")}>{t("清除搜索")}</Button></Flex>
      {search.truncated&&<Text size="1" color="gray">{t("仅显示前 100 项；请缩小搜索范围。")}</Text>}
      <nav aria-label={t("文档搜索结果")}>
        {search.results.map(result=>{
          const label=result.kind==="document"?t("Markdown 文档结果"):t("API 请求结果");
          return <button type="button" key={`${result.kind}/${result.id}`} aria-label={`${label}: ${result.title}`} onClick={()=>openSearchResult(result)}>
            <strong>{result.title}</strong><small>{label} · {result.context||t("工作区根目录")}</small>{result.excerpt&&<span>{result.excerpt}</span>}
          </button>;
        })}
        {!search.total&&<Text size="2" color="gray">{t("没有找到匹配的文档或接口。")}</Text>}
      </nav>
    </Card>}
    {invalidTitleDocument&&<Text role="alert" color="red" size="2">{t("文档「{{value0}}」的标题须为 1–128 个字符，不能含首尾空格或控制字符。",{value0:invalidTitleDocument.name})}</Text>}
    {invalidSourceDocument&&<Text role="alert" color="red" size="2">{t("文档「{{value0}}」的 Markdown 正文不能超过 256 KiB。",{value0:invalidSourceDocument.name})}</Text>}
    {invalidTotalSource&&<Text role="alert" color="red" size="2">{t("工作区 Markdown 正文合计不能超过 2 MiB。")}</Text>}
    <Tabs.Root value={section} onValueChange={setSection} className="documents-sections">
      <Tabs.List><Tabs.Trigger value="markdown">{t("Markdown")}</Tabs.Trigger><Tabs.Trigger value="api">{t("API 参考")}</Tabs.Trigger></Tabs.List>
      <Tabs.Content value="markdown">
    <div className="documents-layout">
      <nav className="documents-list" aria-label={t("文档列表")}>
        {documents.length?documents.map(document=><button type="button" key={document.id} className={document.id===selected?"selected":""} onClick={()=>{setSelectedId(document.id);setMode("preview");}}>
          <span>{document.name}</span><small>{draft.data.collections.find(collection=>collection.id===document.collection_id)?.name??t("工作区根目录")}</small>
        </button>):<Text size="2" color="gray">{t("还没有 Markdown 文档。")}</Text>}
      </nav>
      {current?<Card className="document-editor-card">
        <Flex justify="between" align="center" gap="3" wrap="wrap"><Heading as="h2" ref={documentHeadingRef} tabIndex={-1} size="3">{current.name}</Heading><Button variant="ghost" color="red" onClick={remove} aria-label={t("删除文档")}><Trash2 size={16}/>{t("删除")}</Button></Flex>
        <div className="document-fields">
          <Field label={t("文档标题")}><TextField.Root value={current.name} maxLength={128} onChange={event=>updateDocument({name:event.target.value})}/></Field>
          <Choice label={t("所在目录")} value={current.collection_id??"none"} onChange={value=>updateDocument({collection_id:value==="none"?null:value})} options={[{value:"none",label:t("工作区根目录")},...draft.data.collections.map(collection=>({value:collection.id,label:collection.name}))]}/>
          <label className="document-private"><Checkbox checked={current.secret??true} onCheckedChange={value=>updateDocument({secret:value===true})}/><span>{t("私密文档")}</span></label>
        </div>
        <Text size="1" color="gray">{t("私密文档的正文不进入默认备份；完整备份会保留原文。")}</Text>
        <Tabs.Root value={mode} onValueChange={setMode}>
          <Tabs.List><Tabs.Trigger value="edit">{t("编辑")}</Tabs.Trigger><Tabs.Trigger value="preview">{t("预览")}</Tabs.Trigger></Tabs.List>
          <Tabs.Content value="edit"><Editor label={t("Markdown 正文")} value={current.source} onChange={source=>updateDocument({source})} dark={dark} language="markdown" height="420px"/></Tabs.Content>
          <Tabs.Content value="preview"><MarkdownPreview source={current.source} requestIds={requestIds} onOpenRequest={openRequest}/></Tabs.Content>
        </Tabs.Root>
      </Card>:<Card className="document-editor-card"><Text color="gray">{t("选择文档，或新建一篇说明。")}</Text></Card>}
    </div>
      </Tabs.Content>
      <Tabs.Content value="api"><ApiReference data={draft.data} scopeKey={`${accountId}/${draft.id}`} onOpenRequest={openRequest}/></Tabs.Content>
    </Tabs.Root>
  </div>;
}
