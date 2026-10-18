import {useEffect,useState} from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {Button,Card,Checkbox,Flex,Heading,Tabs,Text,TextField} from "@radix-ui/themes";
import {FilePlus2,Trash2} from "lucide-react";
import {t,useLanguage,message} from "../../shared/i18n";
import {id} from "../../shared/model";
import {Choice,Editor,Field} from "../../shared/ui";
import type {MarkdownDocument} from "../../shared/types";
import {useWorkbench} from "../workbench/context";

function safeLink(raw?:string):string {
  if(!raw)return "";
  try {
    const url=new URL(raw);
    return ["https:","http:","mailto:"].includes(url.protocol)?url.href:"";
  } catch {return "";}
}

export function MarkdownPreview({source}:{source:string}) {
  return <div className="markdown-preview">
    <ReactMarkdown skipHtml remarkPlugins={[remarkGfm]} components={{
      a:({href,children})=>{
        const safe=safeLink(href);
        return safe?<a href={safe} target="_blank" rel="noopener noreferrer">{children}</a>:<span>{children}</span>;
      },
      img:({alt})=><span>{alt||t("图片已省略")}</span>
    }}>{source}</ReactMarkdown>
  </div>;
}

export default function DocumentsPage() {
  useLanguage();
  const state=useWorkbench();
  const {draft,updateData,save,setGuard,dirty,saving,dark,accountId}=state;
  const [selectedId,setSelectedId]=useState("");
  const [mode,setMode]=useState("preview");
  useEffect(()=>{setSelectedId("");setMode("preview");},[accountId,draft?.id]);
  if(!draft)return null;
  const documents=draft.data.documents??[];
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
  const invalidTitle=!!current&&(current.name.trim()!==current.name||!current.name.trim()||[...current.name].length>128||/[\x00-\x1F\x7F]/.test(current.name));
  const sourceBytes=current?new TextEncoder().encode(current.source).length:0;
  const invalidSource=sourceBytes>256*1024;
  return <div className="page-panel documents-page">
    <div className="page-heading">
      <div><Heading size="5">{t("Markdown 文档")}</Heading><Text size="2" color="gray">{t("为工作区编写说明，并在保存前预览。")}</Text></div>
      <Flex gap="2" wrap="wrap"><Button variant="soft" color="gray" onClick={create} disabled={documents.length>=256}><FilePlus2 size={16}/>{t("新建文档")}</Button><Button variant={dirty?"solid":"soft"} color={dirty?undefined:"gray"} onClick={()=>void save()} disabled={!dirty||saving||invalidTitle||invalidSource} loading={saving}>{t("保存工作区")}</Button></Flex>
    </div>
    <div className="documents-layout">
      <nav className="documents-list" aria-label={t("文档列表")}>
        {documents.length?documents.map(document=><button type="button" key={document.id} className={document.id===selected?"selected":""} onClick={()=>{setSelectedId(document.id);setMode("preview");}}>
          <span>{document.name}</span><small>{draft.data.collections.find(collection=>collection.id===document.collection_id)?.name??t("工作区根目录")}</small>
        </button>):<Text size="2" color="gray">{t("还没有 Markdown 文档。")}</Text>}
      </nav>
      {current?<Card className="document-editor-card">
        <Flex justify="between" align="center" gap="3" wrap="wrap"><Heading size="3">{current.name}</Heading><Button variant="ghost" color="red" onClick={remove} aria-label={t("删除文档")}><Trash2 size={16}/>{t("删除")}</Button></Flex>
        <div className="document-fields">
          <Field label={t("文档标题")}><TextField.Root value={current.name} maxLength={128} onChange={event=>updateDocument({name:event.target.value})}/></Field>
          <Choice label={t("所在目录")} value={current.collection_id??"none"} onChange={value=>updateDocument({collection_id:value==="none"?null:value})} options={[{value:"none",label:t("工作区根目录")},...draft.data.collections.map(collection=>({value:collection.id,label:collection.name}))]}/>
          <label className="document-private"><Checkbox checked={current.secret??true} onCheckedChange={value=>updateDocument({secret:value===true})}/><span>{t("私密文档")}</span></label>
        </div>
        <Text size="1" color="gray">{t("私密文档的正文不进入默认备份；完整备份会保留原文。")}</Text>
        {invalidTitle&&<Text role="alert" color="red" size="2">{t("标题须为 1–128 个字符，不能含首尾空格或控制字符。")}</Text>}
        {invalidSource&&<Text role="alert" color="red" size="2">{t("Markdown 正文不能超过 256 KiB。")}</Text>}
        <Tabs.Root value={mode} onValueChange={setMode}>
          <Tabs.List><Tabs.Trigger value="edit">{t("编辑")}</Tabs.Trigger><Tabs.Trigger value="preview">{t("预览")}</Tabs.Trigger></Tabs.List>
          <Tabs.Content value="edit"><Editor label={t("Markdown 正文")} value={current.source} onChange={source=>updateDocument({source})} dark={dark} language="markdown" height="420px"/></Tabs.Content>
          <Tabs.Content value="preview"><MarkdownPreview source={current.source}/></Tabs.Content>
        </Tabs.Root>
      </Card>:<Card className="document-editor-card"><Text color="gray">{t("选择文档，或新建一篇说明。")}</Text></Card>}
    </div>
  </div>;
}
