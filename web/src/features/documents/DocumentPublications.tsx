import {useEffect,useState} from "react";
import {Button,Card,Checkbox,Flex,Heading,Text,TextField} from "@radix-ui/themes";
import {api} from "../../shared/api";
import {message,t,useLanguage} from "../../shared/i18n";
import {safeMessage} from "../../shared/model";
import {useWorkbench} from "../workbench/context";
import {hasChanges} from "../workspaces/draft";

type Publication={id:string;source_revision:number;created_at:number;title:string;document_titles:string[];public_url:string;revoked:boolean};
const newKey=()=>crypto.randomUUID();

export default function DocumentPublications() {
  useLanguage();
  const state=useWorkbench();
  const {draft,accountId,dirty,setGuard,stateRef,accountRef}=state;
  const [versions,setVersions]=useState<Publication[]>([]);
  const [selected,setSelected]=useState<Set<string>>(new Set());
  const [title,setTitle]=useState("");
  const [key,setKey]=useState(newKey);
  const [busy,setBusy]=useState(false);
  const [loading,setLoading]=useState(true);
  const [failure,setFailure]=useState("");
  const [notice,setNotice]=useState("");
  const workspaceId=draft?.id??"";
  useEffect(()=>{
    if(!workspaceId)return;
    let active=true;
    setLoading(true);
    api<Publication[]>(`/api/workspaces/${encodeURIComponent(workspaceId)}/doc-publications`)
      .then(items=>{if(active){setVersions(items);setFailure("");}})
      .catch(error=>{if(active)setFailure(safeMessage(error));})
      .finally(()=>{if(active)setLoading(false);});
    return ()=>{active=false;};
  },[accountId,workspaceId]);
  if(!draft)return null;
  const eligible=(draft.data.documents??[]).filter(document=>!document.secret);
  const choices=eligible.filter(document=>selected.has(document.id));
  const bytes=choices.reduce((sum,document)=>sum+new TextEncoder().encode(document.source).length,0);
  const changeSelection=(id:string,checked:boolean)=>{
    setSelected(current=>{
      const next=new Set(current);
      if(checked)next.add(id);else next.delete(id);
      return next;
    });
    setKey(newKey());setNotice("");
  };
  const publish=()=>{
    const name=title.trim();
    const ids=choices.map(document=>document.id);
    const revision=draft.revision;
    const idempotencyKey=key;
    if(!name||name!==title||[...name].length>128||!ids.length||ids.length>32||bytes>1024*1024||dirty)return;
    setGuard({
      title:message("发布文档快照"),
      description:message("以下文档将无需登录即可查看：{{value0}}。发布后，编辑工作区不会更新此版本。",{value0:choices.map(document=>document.name).join("、")}),
      action:()=>{
        const latest=stateRef.current;
        if(accountRef.current!==accountId||latest.draft?.id!==workspaceId||latest.draft.revision!==revision||hasChanges(latest)){
          setFailure(t("工作区已变化，请保存并重新确认发布内容。"));return;
        }
        setBusy(true);setFailure("");setNotice("");
        void api<Publication>(`/api/workspaces/${encodeURIComponent(workspaceId)}/doc-publications`,"POST",{
          title:name,expected_revision:revision,document_ids:ids,idempotency_key:idempotencyKey
        }).then(created=>{
          if(accountRef.current!==accountId||stateRef.current.draft?.id!==workspaceId)return;
          setVersions(current=>[created,...current.filter(item=>item.id!==created.id)]);
          setSelected(new Set());setTitle("");setKey(newKey());setNotice(t("文档快照已发布。"));
        }).catch(error=>setFailure(safeMessage(error))).finally(()=>setBusy(false));
      }
    });
  };
  const revoke=(version:Publication)=>setGuard({
    title:message("撤销文档发布"),
    description:message("撤销「{{value0}}」后，公开链接将立即失效。",{value0:version.title}),
    action:()=>{
      if(accountRef.current!==accountId||stateRef.current.draft?.id!==workspaceId)return;
      setBusy(true);setFailure("");setNotice("");
      void api(`/api/workspaces/${encodeURIComponent(workspaceId)}/doc-publications/${version.id}`,"DELETE")
        .then(()=>{if(accountRef.current===accountId&&stateRef.current.draft?.id===workspaceId){
          setVersions(current=>current.map(item=>item.id===version.id?{...item,revoked:true}:item));setNotice(t("文档发布已撤销。"));
        }})
        .catch(error=>setFailure(safeMessage(error))).finally(()=>setBusy(false));
    }
  });
  const copy=async(id:string)=>{
    try {
      await navigator.clipboard.writeText(new URL(`/docs/${id}`,window.location.origin).href);
      setNotice(t("公开链接已复制。"));
    } catch {setFailure(t("无法复制公开链接。"));}
  };
  return <section className="document-publications" aria-label={t("文档发布")}>
    <Heading as="h2" size="4">{t("文档发布")}</Heading>
    <Text size="2" color="gray">{t("只发布明确选中的非私密 Markdown。公开版本固定不变，可随时撤销。")}</Text>
    <Card>
      <Flex direction="column" gap="3">
        <label className="field"><Text size="2">{t("公开站点标题")}</Text><TextField.Root value={title} maxLength={128} onChange={event=>{setTitle(event.target.value);setKey(newKey());}}/></label>
        <fieldset className="document-publications-choices"><legend>{t("选择要发布的文档")}</legend>
          {eligible.length?eligible.map(document=><label key={document.id}><Checkbox checked={selected.has(document.id)} onCheckedChange={checked=>changeSelection(document.id,checked===true)}/><span>{document.name}</span></label>):<Text size="2" color="gray">{t("没有可发布的非私密文档。")}</Text>}
        </fieldset>
        <Text size="2" color="gray">{t("发布内容无需登录即可查看。请先检查文档原文中的凭证和个人信息。")}</Text>
        {dirty&&<Text role="alert" size="2" color="amber">{t("请先保存工作区，再发布固定版本。")}</Text>}
        {bytes>1024*1024&&<Text role="alert" size="2" color="red">{t("所选文档正文合计不能超过 1 MiB。")}</Text>}
        <Button disabled={busy||loading||dirty||versions.filter(version=>!version.revoked).length>=20||versions.length>=256||!title.trim()||title.trim()!==title||choices.length<1||choices.length>32||bytes>1024*1024} onClick={publish}>{t("发布所选文档")}</Button>
      </Flex>
    </Card>
    {failure&&<Text role="alert" color="red" size="2">{failure}</Text>}
    {notice&&<Text role="status" color="green" size="2">{notice}</Text>}
    <Heading as="h3" size="3">{t("已发布版本")}</Heading>
    {loading?<Text role="status" size="2">{t("正在读取发布版本…")}</Text>:versions.length?versions.map(version=><Card key={version.id}>
      <Flex justify="between" align="center" gap="3" wrap="wrap"><div><Text weight="medium">{version.title}</Text><Text as="p" size="1" color="gray">{version.document_titles.join("、")} · {new Date(version.created_at*1000).toLocaleString()}</Text></div><Flex gap="2" wrap="wrap">{version.revoked?<Text size="2" color="gray">{t("已撤销")}</Text>:<><Button asChild variant="soft"><a href={`/docs/${version.id}`} target="_blank" rel="noopener noreferrer">{t("打开公开页")}</a></Button><Button variant="soft" onClick={()=>void copy(version.id)}>{t("复制链接")}</Button><Button variant="soft" color="red" disabled={busy} onClick={()=>revoke(version)}>{t("撤销发布")}</Button></>}</Flex></Flex>
    </Card>):<Text size="2" color="gray">{t("尚未发布文档。")}</Text>}
  </section>;
}
