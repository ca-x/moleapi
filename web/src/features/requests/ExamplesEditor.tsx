import { useEffect, useMemo, useRef, useState } from "react";
import { t, useLanguage } from "../../shared/i18n";
import { Badge, Button, Card, Checkbox, Dialog, Flex, Select, Text, TextArea, TextField } from "@radix-ui/themes";
import { Trash2 } from "lucide-react";
import { api, native, previewMock, token } from "../../shared/api";
import { safeMessage } from "../../shared/model";
import { Editor, Field, PairEditor, ToolButton } from "../../shared/ui";
import type { RequestSpec, Specification } from "../../shared/types";
import MockCallActivity from "./MockCallActivity";
import MockPublicShares from "./MockPublicShares";
import {openapiMockChoices} from "./openapiMockChoices";

function invalidSchemaInput(source:string):boolean {
  if(new TextEncoder().encode(source).length>256*1024)return true;
  try { const value=JSON.parse(source); return !value||typeof value!=="object"||Array.isArray(value); }
  catch { return true; }
}

function SchemaEditorDialog({schema,dark,onCommit}:{schema:string;dark:boolean;onCommit:(schema:string)=>void}) {
  const [open,setOpen]=useState(false);
  const [draft,setDraft]=useState(schema);
  return <Dialog.Root open={open} onOpenChange={next=>{if(next)setDraft(schema);setOpen(next);}}>
    <Dialog.Trigger><Button variant="soft">{t("编辑 Mock JSON Schema")}</Button></Dialog.Trigger>
    <Dialog.Content maxWidth="min(720px, calc(100vw - 24px))" style={{maxHeight:"calc(100dvh - 24px)",overflowY:"auto"}}>
      <Dialog.Title>{t("Mock JSON Schema")}</Dialog.Title>
      <Dialog.Description>{t("保存后使用下方预览；支持基本类型、属性、数组和本地 $defs 引用。原静态正文保留。")}</Dialog.Description>
      <Editor label={t("Mock JSON Schema")} value={draft} onChange={setDraft} dark={dark} language="json" height="320px" />
      {invalidSchemaInput(draft) && <Text role="alert" size="1" color="red">{t("Schema 必须是 256 KiB 内的 JSON 对象；保存时会检查支持的规则。")}</Text>}
      <Flex justify="end" mt="4"><Dialog.Close><Button variant="soft" color="gray" disabled={invalidSchemaInput(draft)} onClick={()=>onCommit(draft)}>{t("完成")}</Button></Dialog.Close></Flex>
    </Dialog.Content>
  </Dialog.Root>;
}
export default function ExamplesEditor({
  request,
  update,
  dark,
  asyncMock,
  workspaceId,
  workspaceRevision,
  specifications = [],
  dirty = false,
}: {
  request: RequestSpec;
  update: (patch: Partial<RequestSpec>) => void;
  dark: boolean;
  asyncMock?: boolean;
  workspaceId?: string;
  workspaceRevision?: number;
  specifications?: Specification[];
  dirty?: boolean;
}) {
  useLanguage();
  const [preview,setPreview]=useState<{scope:string;fingerprint:string;status:number;body:string}|null>(null);
  const [busy,setBusy]=useState("");
  const [error,setError]=useState("");
  const [selectionError,setSelectionError]=useState<{id:string;message:string}|null>(null);
  const [selectionBusy,setSelectionBusy]=useState("");
  const selectionVersion=useRef(0);
  const latestRequest=useRef(request);
  latestRequest.current=request;
  const linkedSpec=specifications.find(spec=>spec.id===request.specification_id&&spec.kind==="openapi");
  const linkedChoices=useMemo(()=>linkedSpec?openapiMockChoices(linkedSpec.source,request.method):[],[linkedSpec?.source,request.method]);
  useEffect(()=>{selectionVersion.current++;setSelectionBusy("");},[request.id,request.method,request.specification_id,linkedSpec?.source,workspaceRevision]);
  const validLink=(example:RequestSpec["examples"][number])=>!!linkedSpec&&example.mock_schema_link?.specification_id===linkedSpec.id&&linkedChoices.some(choice=>choice.pointer===example.mock_schema_link?.pointer);
  const patchExample=(exampleId:string,patch:Partial<RequestSpec["examples"][number]>)=>{
    setPreview(null);setError("");
    update({examples:latestRequest.current.examples.map(item=>item.id===exampleId?{...item,...patch}:item)});
  };
  let routePath="";
  let routeHasQuery=request.query.some(row=>row.enabled);
  try{
    const savedUrl=new URL(request.url);
    routePath=savedUrl.pathname.replace(/%7B/gi,"{").replace(/%7D/gi,"}");
    routeHasQuery=routeHasQuery||savedUrl.search!=="";
    if(routePath.split("/").some(segment=>/[{}]/.test(segment)&&!/^[{][A-Za-z_][A-Za-z0-9_]{0,63}[}]$/.test(segment)))routePath="";
  }catch{routePath="";}
  const previewExample=(exampleId:string)=>{
    if(!workspaceId||dirty)return;
    const scope=`${workspaceId}/${request.id}/${exampleId}`;
    const fingerprint=JSON.stringify(request.examples.find(example=>example.id===exampleId))??"";
    const identity=token();
    setBusy(scope);setError("");setPreview(null);
    void previewMock(`/api/mock/${[workspaceId,request.id,exampleId].map(encodeURIComponent).join("/")}`)
      .then(result=>{if(token()===identity)setPreview({scope,fingerprint,...result});})
      .catch(()=>{if(token()===identity)setError(scope);})
      .finally(()=>{if(token()===identity)setBusy("");});
  };
  return (
    <div className="example-list">
      {workspaceId&&<Dialog.Root>
        <Dialog.Trigger><Button variant="soft" style={{minHeight:44}}>{t("管理公开 Mock")}</Button></Dialog.Trigger>
        <Dialog.Content maxWidth="min(560px, calc(100vw - 24px))" style={{maxHeight:"calc(100dvh - 24px)",overflowY:"auto"}}>
          <Dialog.Title>{t("公开 Mock")}</Dialog.Title>
          <Dialog.Description>{native?t("本地工作区默认不监听；启动后，任何持有公开地址的人都可以调用当前请求。"):t("默认仅工作区成员可访问。公开地址只分享当前请求；任何持有地址的人都可以调用。")}</Dialog.Description>
          <MockPublicShares workspaceId={workspaceId} requestId={request.id} examples={request.examples.map(example=>({id:example.id,name:example.name,dynamic:!!example.mock_script?.length}))} routePath={routePath} hasSavedQuery={routeHasQuery} dirty={dirty}/>
          <Flex justify="end" mt="4"><Dialog.Close><Button variant="soft">{t("完成")}</Button></Dialog.Close></Flex>
        </Dialog.Content>
      </Dialog.Root>}
      {request.examples.length === 0 && (
        <Text size="2" color="gray"> {t("发送请求后，点击响应面板的「保存示例」。")} </Text>
      )}
      {request.examples.map((example) => (
        <Card key={example.id}>
          <Flex justify="between" align="center">
            <TextField.Root
              aria-label={t("示例名称")}
              value={example.name}
              onChange={(e) =>
                update({
                  examples: request.examples.map((x) =>
                    x.id === example.id ? { ...x, name: e.target.value } : x,
                  ),
                })
              }
            />
            <Flex gap="3">
              <Badge color="gray">{example.status}</Badge>
              <ToolButton
                label={t("删除示例")}
                onClick={() =>
                  update({
                    examples: request.examples.filter(
                      (x) => x.id !== example.id,
                    ),
                  })
                }
              >
                <Trash2 size={15} />
              </ToolButton>
            </Flex>
          </Flex>
          <Field label={t("Mock 延迟（毫秒）")} hint={t("0 表示立即响应；最多等待 30000 毫秒。")}><TextField.Root type="number" min="0" max="30000" step="1" value={example.delay_ms??0} onChange={event=>{
            const value=Number(event.target.value);
            if(Number.isFinite(value))patchExample(example.id,{delay_ms:Math.min(30_000,Math.max(0,Math.round(value)))});
          }}/></Field>
          <Editor value={example.body} readOnly dark={dark} height="150px" />
          {linkedSpec ? <Field label={t("Mock 正文来源")} hint={t("来源规范：{{name}}",{name:linkedSpec.name})}>
            <Select.Root value={example.mock_schema_link?(validLink(example)?example.mock_schema_link.pointer:"stale"):(example.mock_schema?"inline":"static")} onValueChange={value=>{
              const version=++selectionVersion.current;
              setSelectionError(null);
              if(value==="static")patchExample(example.id,{mock_schema:undefined,mock_schema_link:undefined});
              else if(value==="inline")patchExample(example.id,{mock_schema:example.mock_schema||'{"type":"object","properties":{}}',mock_schema_link:undefined});
              else if(value!=="stale") {
                const chosen=linkedChoices.find(choice=>choice.pointer===value);
                if(!chosen)return;
                if(!workspaceId||dirty||workspaceRevision===undefined){setSelectionError({id:example.id,message:t("先保存工作区，再选择关联响应。")});return;}
                const identity=token();
                setSelectionBusy(example.id);
                void api<{method:string;status:number;media_type:string}>(`/api/workspaces/${encodeURIComponent(workspaceId)}/mock-schema/validate`,"POST",{request_id:request.id,specification_id:linkedSpec.id,pointer:value,expected_revision:workspaceRevision})
                  .then(result=>{if(selectionVersion.current===version&&token()===identity&&latestRequest.current.method===request.method&&latestRequest.current.specification_id===linkedSpec.id){const current=latestRequest.current.examples.find(item=>item.id===example.id);if(current)patchExample(example.id,{status:result.status,headers:current.headers.map(header=>header.enabled&&header.key.toLowerCase()==="content-type"?{...header,value:result.media_type}:header),mock_schema:undefined,mock_schema_link:{specification_id:linkedSpec.id,pointer:value}});}})
                  .catch(reason=>{if(selectionVersion.current===version&&token()===identity)setSelectionError({id:example.id,message:t("无法使用所选响应 Schema：{{reason}}",{reason:safeMessage(reason)})});})
                  .finally(()=>{if(selectionVersion.current===version&&token()===identity)setSelectionBusy("");});
              }
            }}>
              <Select.Trigger aria-label={t("Mock 正文来源")} />
              <Select.Content>
                <Select.Item value="static">{t("已保存的静态正文")}</Select.Item>
                <Select.Item value="inline">{t("内联 JSON Schema")}</Select.Item>
                {linkedChoices.map(choice=><Select.Item key={choice.pointer} value={choice.pointer}>{choice.label}</Select.Item>)}
                {example.mock_schema_link&&!validLink(example)&&<Select.Item value="stale">{t("已关联的响应不可用")}</Select.Item>}
              </Select.Content>
            </Select.Root>
          </Field> : <Text as="label" size="2"><Checkbox checked={!!example.mock_schema} onCheckedChange={checked=>patchExample(example.id,{mock_schema:checked===true?'{"type":"object","properties":{}}':undefined,mock_schema_link:undefined})}/> {t("根据 JSON Schema 生成 Mock 正文")}</Text>}
          {linkedSpec&&linkedChoices.length===0&&<Text size="1" color="gray">{t("规范中没有可选的 JSON 响应。")}</Text>}
          {example.mock_schema_link&&!validLink(example)&&<><Text role="alert" size="1" color="red">{linkedSpec?t("关联的响应与当前规范不匹配；请重新选择。") :t("关联的 OpenAPI 规范不可用；请选择静态正文或内联 Schema。")}</Text><Button variant="soft" color="gray" onClick={()=>patchExample(example.id,{mock_schema_link:undefined})}>{t("清除 Schema 关联")}</Button></>}
          {selectionBusy===example.id&&<Text size="1" color="gray">{t("正在校验关联响应…")}</Text>}
          {selectionError?.id===example.id&&<Text role="alert" size="1" color="red">{selectionError.message}</Text>}
          {!!example.mock_schema && <SchemaEditorDialog schema={example.mock_schema} dark={dark} onCommit={mock_schema=>patchExample(example.id,{mock_schema})}/>}
          {workspaceId && <Text as="p" size="1" color="gray" style={{overflowWrap:"anywhere"}}>{t("Mock 地址")}: <code>{`/api/mock/${encodeURIComponent(workspaceId)}/${encodeURIComponent(request.id)}/${encodeURIComponent(example.id)}`}</code></Text>}
          {workspaceId && routePath && <Text as="p" size="1" color="gray" style={{overflowWrap:"anywhere"}}>{t("匹配路由")}: <code>{`${request.method} /api/mocks/${encodeURIComponent(workspaceId)}${routePath}`}</code></Text>}
          {workspaceId && !routePath && <Text size="1" color="gray">{t("Mock 匹配路由需要字面量 HTTP URL；路径变量使用单层 {name}。")}</Text>}
          {workspaceId && <Button variant="soft" disabled={dirty||busy!==""} loading={busy===`${workspaceId}/${request.id}/${example.id}`} onClick={()=>previewExample(example.id)}>{t("预览 Mock")}</Button>}
          {dirty && <Text size="1" color="gray">{t("先保存工作区，再预览 Mock。")}</Text>}
          {error===`${workspaceId}/${request.id}/${example.id}` && <Text role="alert" size="1" color="red">{t("Mock 预览失败，请检查脚本与数据源。")}</Text>}
          {!dirty && preview?.scope===`${workspaceId}/${request.id}/${example.id}` && preview.fingerprint===JSON.stringify(example) && <Field label={t("Mock 预览结果")} hint={`${t("状态码")}: ${preview.status}`}><TextArea readOnly rows={5} value={preview.body.slice(0,64*1024)} /></Field>}
          {!example.mock_match ? <Button variant="soft" onClick={()=>patchExample(example.id,{mock_match:{priority:0,query:[],headers:[],body:""}})}>{t("添加 Mock 匹配条件")}</Button> : <div>
            <Text size="1" color="gray">{t("匹配条件全部满足时选中此示例；空正文只匹配无正文请求。")}</Text>
            <Field label={t("匹配优先级")}><TextField.Root type="number" min="0" max="100" value={example.mock_match.priority} onChange={event=>patchExample(example.id,{mock_match:{...example.mock_match!,priority:Number(event.target.value)}})}/></Field>
            <Field label={t("查询参数条件")}><PairEditor rows={example.mock_match.query??[]} onChange={query=>patchExample(example.id,{mock_match:{...example.mock_match!,query}})} keyLabel={t("参数名")} valueLabel={t("参数值")} secrets/></Field>
            <Field label={t("请求头条件")}><PairEditor rows={example.mock_match.headers??[]} onChange={headers=>patchExample(example.id,{mock_match:{...example.mock_match!,headers}})} keyLabel="Header" valueLabel={t("值")} secrets/></Field>
            <Text as="label" size="2"><Checkbox checked={example.mock_match.body!==undefined} onCheckedChange={checked=>patchExample(example.id,{mock_match:{...example.mock_match!,body:checked===true?"":undefined}})}/>{t("匹配正文")}</Text>
            {example.mock_match.body!==undefined && <Field label={t("精确匹配正文")}><TextArea value={example.mock_match.body} onChange={event=>patchExample(example.id,{mock_match:{...example.mock_match!,body:event.target.value}})} rows={3}/></Field>}
            <Button variant="ghost" color="red" onClick={()=>patchExample(example.id,{mock_match:undefined})}>{t("移除匹配条件")}</Button>
          </div>}
          {asyncMock ? <Field label={t("Mock 实现脚本")} hint={t("使用 pm.mock 设置状态、响应头与正文；可通过 pm.datasets(ID) 查询工作区数据集。") }><Editor label={t("Mock 实现脚本")} value={example.mock_script??""} onChange={mock_script=>{setPreview(null);setError("");update({examples:request.examples.map(item=>item.id===example.id?{...item,mock_script}:item)});}} dark={dark} language="javascript" height="180px"/></Field> : <Text size="1" color="gray">{t("在工作区设置中启用 v2 脚本模式后可编辑动态 Mock。")}</Text>}
        </Card>
      ))}
      {workspaceId&&!dirty&&<MockCallActivity workspaceId={workspaceId} requestId={request.id}/>}
    </div>
  );
}
