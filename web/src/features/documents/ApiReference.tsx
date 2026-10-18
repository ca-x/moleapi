import {useEffect,useState} from "react";
import {Badge,Card,Flex,Heading,Text} from "@radix-ui/themes";
import {t,useLanguage} from "../../shared/i18n";
import type {Auth,Pair,WorkspaceData} from "../../shared/types";
import MarkdownPreview from "./MarkdownPreview";

function authLabel(kind:Auth["kind"]):string {
  const labels:Record<Auth["kind"],string>={inherit:t("继承父级鉴权"),none:"No Auth",bearer:"Bearer Token",basic:"Basic Auth",apikey:"API Key",jwt:"JWT",digest:"Digest Auth",oauth2:"OAuth 2.0",aws:"AWS Signature V4",hawk:"Hawk",oauth1:"OAuth 1.0",ntlm:"NTLM",edgegrid:"Akamai EdgeGrid",asap:"Atlassian ASAP"};
  return labels[kind];
}

function PairTable({rows,label,nested=false}:{rows:Pair[];label:string;nested?:boolean}) {
  const enabled=rows.filter(row=>row.enabled);
  if(!enabled.length)return null;
  return <section className="api-reference-section">
    <Heading as={nested?"h5":"h3"} size={nested?"2":"3"}>{label}</Heading>
    <div className="api-reference-table-scroll"><table>
      <thead><tr><th scope="col">{t("名称")}</th><th scope="col">{t("示例值")}</th></tr></thead>
      <tbody>{enabled.map(row=><tr key={row.id}><th scope="row">{row.key}</th><td>{row.value}</td></tr>)}</tbody>
    </table></div>
  </section>;
}

export default function ApiReference({data,scopeKey}:{data:WorkspaceData;scopeKey:string}) {
  useLanguage();
  const [selection,setSelection]=useState({scopeKey,id:"",index:0});
  const groups=data.collections.map(collection=>({collection,requests:collection.requests.filter(request=>!request.protocol||request.protocol.kind==="http")}));
  const entries=groups.flatMap(group=>group.requests.map(request=>({collection:group.collection,request})));
  const omitted=data.collections.reduce((count,collection)=>count+collection.requests.length,0)-entries.length;
  const active=selection.scopeKey===scopeKey?selection:{scopeKey,id:"",index:0};
  const currentIndex=entries.findIndex(entry=>entry.request.id===active.id);
  const resolvedIndex=currentIndex>=0?currentIndex:Math.min(active.index,entries.length-1);
  const selected=entries[resolvedIndex];
  useEffect(()=>{
    const id=selected?.request.id??"";
    if(selection.scopeKey!==scopeKey||selection.id!==id||selection.index!==Math.max(0,resolvedIndex))setSelection({scopeKey,id,index:Math.max(0,resolvedIndex)});
  },[scopeKey,selection,selected?.request.id,resolvedIndex]);
  const request=selected?.request;
  return <div className="api-reference">
    <div className="api-reference-intro">
      <Heading as="h2" size="4">{t("自动生成的 API 文档")}</Heading>
      <Text size="2" color="gray">{t("仅在当前工作区预览；内容可能包含保存的凭证和示例数据。")}</Text>
      {omitted>0&&<Text size="2" color="gray">{t("{{value0}} 个非 HTTP 请求未包含在此文档中。",{value0:omitted})}</Text>}
    </div>
    <div className="documents-layout">
      <nav className="documents-list api-reference-list" aria-label={t("API 文档请求列表")}>
        {groups.map(({collection,requests})=><section key={collection.id}>
          <Heading as="h3" size="2">{collection.name}</Heading>
          {requests.length?requests.map(item=><button type="button" key={item.id} className={item.id===request?.id?"selected":""} aria-current={item.id===request?.id?"true":undefined} onClick={()=>setSelection({scopeKey,id:item.id,index:entries.findIndex(entry=>entry.request.id===item.id)})}>
            <span>{item.method} {item.name}</span>
          </button>):<Text size="1" color="gray">{t("此目录没有 HTTP 请求。")}</Text>}
        </section>)}
      </nav>
      {request?<Card className="document-editor-card api-reference-article">
        <article>
          <Flex gap="2" align="center" wrap="wrap"><Badge color="gray">{request.method}</Badge><Heading as="h2" size="4">{request.name}</Heading></Flex>
          <Text size="1" color="gray">{selected.collection.name}</Text>
          <pre className="api-reference-source">{request.url}</pre>
          {request.description&&<section className="api-reference-section"><Heading as="h3" size="3">{t("说明")}</Heading><MarkdownPreview source={request.description} nested/></section>}
          <PairTable rows={request.query} label={t("查询参数")}/>
          <PairTable rows={request.headers} label={t("请求头")}/>
          <section className="api-reference-section"><Heading as="h3" size="3">{t("鉴权")}</Heading><Text>{authLabel(request.auth.kind)}</Text></section>
          {request.body_kind!=="none"&&<section className="api-reference-section">
            <Heading as="h3" size="3">{t("请求体")}</Heading>
            <Text size="2" color="gray">{request.body_kind}</Text>
            {["json","text","form"].includes(request.body_kind)?<pre className="api-reference-source">{request.body}</pre>:<Text size="2" color="gray">{t("二进制与多部分正文不在此处预览。")}</Text>}
          </section>}
          {!!request.examples.length&&<section className="api-reference-section">
            <Heading as="h3" size="3">{t("响应示例")}</Heading>
            {request.examples.map(example=><div className="api-reference-example" key={example.id}>
              <Heading as="h4" size="2">{example.status} {example.name}</Heading>
              <PairTable rows={example.headers} label={t("响应头")} nested/>
              <pre className="api-reference-source">{example.body}</pre>
            </div>)}
          </section>}
        </article>
      </Card>:<Card className="document-editor-card"><Text color="gray">{t("还没有可生成文档的 HTTP 请求。")}</Text></Card>}
    </div>
  </div>;
}
