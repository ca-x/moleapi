import {useEffect,useMemo,useRef,useState} from "react";
import type {FormEvent} from "react";
import {Button,Heading,Text,TextField,Theme} from "@radix-ui/themes";
import MarkdownPreview from "./MarkdownPreview";
import {t,useLanguage} from "../../shared/i18n";
import {indexPublishedDocuments,searchPublishedDocuments} from "./publishedSearch";

type PublishedDocument={title:string;folder:string|null;source:string};
type Appearance={theme:"light"|"dark"|"system";layout:"sidebar"|"top";logo_png_base64?:string};
type Snapshot={title:string;documents:PublishedDocument[];appearance?:Appearance};

function validSnapshot(value:unknown):value is Snapshot {
  if(!value||typeof value!=="object")return false;
  const snapshot=value as Partial<Snapshot>;
  const appearance=snapshot.appearance;
  return (appearance===undefined||(appearance!==null&&["light","dark","system"].includes(appearance.theme)&&["sidebar","top"].includes(appearance.layout)&&(appearance.logo_png_base64===undefined||typeof appearance.logo_png_base64==="string")))&&typeof snapshot.title==="string"&&Array.isArray(snapshot.documents)&&snapshot.documents.length>0&&snapshot.documents.length<=32&&snapshot.documents.every(document=>
    document&&typeof document.title==="string"&&(document.folder===null||typeof document.folder==="string")&&typeof document.source==="string"
  );
}

export default function PublicDocsPage({id}:{id:string}) {
  const {language,setLanguage}=useLanguage();
  const [snapshot,setSnapshot]=useState<Snapshot|null>(null);
  const [status,setStatus]=useState<"loading"|"ready"|"locked"|"missing">("loading");
  const [selected,setSelected]=useState(0);
  const [query,setQuery]=useState("");
  const [password,setPassword]=useState("");
  const [unlocking,setUnlocking]=useState(false);
  const [unlockError,setUnlockError]=useState("");
  const [systemDark,setSystemDark]=useState(()=>window.matchMedia?.("(prefers-color-scheme: dark)").matches??false);
  const activeId=useRef(id);
  const unlockRequest=useRef<AbortController|null>(null);
  activeId.current=id;
  useEffect(()=>{
    const media=window.matchMedia?.("(prefers-color-scheme: dark)");
    if(!media)return;
    const update=()=>setSystemDark(media.matches);
    media.addEventListener("change",update);
    return ()=>media.removeEventListener("change",update);
  },[]);
  useEffect(()=>{
    if(!/^[a-f0-9]{32}$/.test(id)){setStatus("missing");return;}
    const controller=new AbortController();
    unlockRequest.current?.abort();setPassword("");setUnlockError("");setSnapshot(null);setStatus("loading");
    fetch(`/api/public/docs/${id}`,{credentials:"omit",cache:"no-store",signal:controller.signal})
      .then(async response=>({status:response.status,value:response.ok?await response.json():null}))
      .then(({status:code,value})=>{
        if(controller.signal.aborted)return;
        if(validSnapshot(value)){setSnapshot(value);setSelected(0);setQuery("");setStatus("ready");}
        else if(code===403)setStatus("locked");
        else setStatus("missing");
      })
      .catch(()=>{if(!controller.signal.aborted)setStatus("missing");});
    return ()=>{controller.abort();unlockRequest.current?.abort();};
  },[id]);
  useEffect(()=>{
    if(snapshot&&status==="ready")document.title=`${snapshot.title} · MoleAPI`;
    else if(status==="locked")document.title=`${t("受密码保护的文档")} · MoleAPI`;
  },[snapshot,status,language]);
  const unlock=async(event:FormEvent)=>{
    event.preventDefault();
    if(unlocking||!password||new TextEncoder().encode(password).length>128)return;
    const candidate=password;
    setPassword("");setUnlockError("");setUnlocking(true);
    const controller=new AbortController();
    unlockRequest.current?.abort();unlockRequest.current=controller;
    try {
      const response=await fetch(`/api/public/docs/${id}/unlock`,{
        method:"POST",credentials:"omit",cache:"no-store",headers:{"Content-Type":"application/json"},body:JSON.stringify({password:candidate}),signal:controller.signal
      });
      if(controller.signal.aborted||activeId.current!==id)return;
      if(response.status===404){setStatus("missing");return;}
      if(response.status===429){
        const seconds=Number(response.headers.get("Retry-After"));
        setUnlockError(Number.isInteger(seconds)&&seconds>0&&seconds<=300?t("错误次数过多，请在 {{value0}} 秒后重试。",{value0:seconds}):t("错误次数过多，请稍后重试。"));return;
      }
      if(response.status===401){setUnlockError(t("密码不正确，请重试。"));return;}
      if(!response.ok){setUnlockError(t("无法解锁文档，请稍后重试。"));return;}
      const value=await response.json();
      if(validSnapshot(value)){setSnapshot(value);setSelected(0);setQuery("");setStatus("ready");}
      else setUnlockError(t("无法解锁文档，请稍后重试。"));
    } catch {if(!controller.signal.aborted&&activeId.current===id)setUnlockError(t("无法解锁文档，请稍后重试。"));}
    finally {if(activeId.current===id)setUnlocking(false);}
  };
  const current=snapshot?.documents[selected];
  const searchIndex=useMemo(()=>snapshot?indexPublishedDocuments(snapshot.documents):[],[snapshot]);
  const matches=useMemo(()=>searchPublishedDocuments(searchIndex,query),[searchIndex,query]);
  const appearance=snapshot?.appearance;
  const color=appearance?.theme==="dark"||(appearance?.theme==="system"&&systemDark)?"dark":"light";
  return <Theme accentColor="cyan" grayColor="slate" radius="medium" appearance={status==="ready"?color:"light"}>
    <main className="published-docs">
      {status==="loading"?<Text role="status">{t("正在加载文档…")}</Text>:status==="missing"?<div className="published-docs-empty"><Heading size="5">{t("文档不可用")}</Heading><Text>{t("此发布版本不存在，或已被撤销。")}</Text></div>:status==="locked"?<div className="published-docs-lock">
        <div className="published-docs-header-top"><Text size="2" color="gray">MoleAPI</Text><Button size="1" color="gray" variant="ghost" onClick={()=>void setLanguage(language==="en"?"zh-CN":"en")}>{language==="en"?t("简体中文"):"English"}</Button></div>
        <section className="published-docs-lock-card"><form autoComplete="off" onSubmit={event=>void unlock(event)} className="published-docs-lock-form">
          <Heading as="h1" size="5">{t("受密码保护的文档")}</Heading><Text size="2" color="gray">{t("输入发布者提供的密码以查看此文档。")}</Text>
          <label className="field"><Text size="2">{t("访问密码")}</Text><TextField.Root type="password" autoComplete="off" value={password} maxLength={128} onChange={event=>setPassword(event.target.value)}/></label>
          {unlockError&&<Text role="alert" color="red" size="2">{unlockError}</Text>}
          <Button type="submit" disabled={unlocking||!password||new TextEncoder().encode(password).length>128} loading={unlocking}>{t("解锁文档")}</Button>
        </form></section>
      </div>:snapshot&&current?<>
        <header className="published-docs-header"><div className="published-docs-header-top"><Text size="2" color="gray">MoleAPI</Text><Button size="1" color="gray" variant="ghost" onClick={()=>void setLanguage(language==="en"?"zh-CN":"en")}>{language==="en"?t("简体中文"):"English"}</Button></div>{appearance?.logo_png_base64&&<img className="published-docs-logo" src={`data:image/png;base64,${appearance.logo_png_base64}`} alt={snapshot.title}/>}<Heading as="h1" size="6">{snapshot.title}</Heading></header>
        <div className="published-docs-search"><TextField.Root aria-label={t("搜索公开文档")} placeholder={t("搜索公开文档")} value={query} maxLength={128} onChange={event=>setQuery(event.target.value)}/>{query.trim()&&<Text role="status" size="2" color="gray">{matches.length?t("找到 {{value0}} 篇公开文档。",{value0:matches.length}):t("没有匹配的公开文档。")}</Text>}</div>
        <div className={`published-docs-layout${appearance?.layout==="top"?" published-docs-layout-top":""}`}>
          <nav aria-label={t("文档目录")} className="published-docs-nav">
            {matches.map(index=>{const item=snapshot.documents[index];return <Button key={index} variant={selected===index?"soft":"ghost"} color="gray" aria-current={selected===index?"page":undefined} onClick={()=>setSelected(index)}>
              <span>{item.title}</span>{item.folder&&<small>{item.folder}</small>}
            </Button>;})}
          </nav>
          <article className="published-docs-article"><Heading as="h2" size="5">{current.title}</Heading>{current.folder&&<Text size="2" color="gray">{current.folder}</Text>}<MarkdownPreview source={current.source}/></article>
        </div>
      </>:null}
    </main>
  </Theme>;
}
