import {useEffect,useState} from "react";
import {Button,Heading,Text,Theme} from "@radix-ui/themes";
import MarkdownPreview from "./MarkdownPreview";
import {t,useLanguage} from "../../shared/i18n";

type PublishedDocument={title:string;folder:string|null;source:string};
type Snapshot={title:string;documents:PublishedDocument[]};

function validSnapshot(value:unknown):value is Snapshot {
  if(!value||typeof value!=="object")return false;
  const snapshot=value as Partial<Snapshot>;
  return typeof snapshot.title==="string"&&Array.isArray(snapshot.documents)&&snapshot.documents.length>0&&snapshot.documents.length<=32&&snapshot.documents.every(document=>
    document&&typeof document.title==="string"&&(document.folder===null||typeof document.folder==="string")&&typeof document.source==="string"
  );
}

export default function PublicDocsPage({id}:{id:string}) {
  useLanguage();
  const [snapshot,setSnapshot]=useState<Snapshot|null>(null);
  const [status,setStatus]=useState<"loading"|"ready"|"missing">("loading");
  const [selected,setSelected]=useState(0);
  useEffect(()=>{
    if(!/^[a-f0-9]{32}$/.test(id)){setStatus("missing");return;}
    const controller=new AbortController();
    setStatus("loading");
    fetch(`/api/public/docs/${id}`,{credentials:"omit",cache:"no-store",signal:controller.signal})
      .then(async response=>response.ok?response.json():null)
      .then(value=>{
        if(controller.signal.aborted)return;
        if(validSnapshot(value)){setSnapshot(value);setSelected(0);setStatus("ready");document.title=`${value.title} · MoleAPI`;}
        else setStatus("missing");
      })
      .catch(()=>{if(!controller.signal.aborted)setStatus("missing");});
    return ()=>controller.abort();
  },[id]);
  const current=snapshot?.documents[selected];
  return <Theme accentColor="cyan" grayColor="slate" radius="medium">
    <main className="published-docs">
      {status==="loading"?<Text role="status">{t("正在加载文档…")}</Text>:status==="missing"?<div className="published-docs-empty"><Heading size="5">{t("文档不可用")}</Heading><Text>{t("此发布版本不存在，或已被撤销。")}</Text></div>:snapshot&&current?<>
        <header className="published-docs-header"><Text size="2" color="gray">MoleAPI</Text><Heading as="h1" size="6">{snapshot.title}</Heading></header>
        <div className="published-docs-layout">
          <nav aria-label={t("文档目录")} className="published-docs-nav">
            {snapshot.documents.map((item,index)=><Button key={index} variant={selected===index?"soft":"ghost"} color="gray" aria-current={selected===index?"page":undefined} onClick={()=>setSelected(index)}>
              <span>{item.title}</span>{item.folder&&<small>{item.folder}</small>}
            </Button>)}
          </nav>
          <article className="published-docs-article"><Heading as="h2" size="5">{current.title}</Heading>{current.folder&&<Text size="2" color="gray">{current.folder}</Text>}<MarkdownPreview source={current.source}/></article>
        </div>
      </>:null}
    </main>
  </Theme>;
}
