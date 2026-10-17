import { useState } from "react";
import { t, useLanguage } from "../../shared/i18n";
import { Badge, Button, Card, Checkbox, Flex, Text, TextArea, TextField } from "@radix-ui/themes";
import { Trash2 } from "lucide-react";
import { previewMock, token } from "../../shared/api";
import { Editor, Field, PairEditor, ToolButton } from "../../shared/ui";
import type { RequestSpec } from "../../shared/types";
import MockCallActivity from "./MockCallActivity";
export default function ExamplesEditor({
  request,
  update,
  dark,
  asyncMock,
  workspaceId,
  dirty = false,
}: {
  request: RequestSpec;
  update: (patch: Partial<RequestSpec>) => void;
  dark: boolean;
  asyncMock?: boolean;
  workspaceId?: string;
  dirty?: boolean;
}) {
  useLanguage();
  const [preview,setPreview]=useState<{scope:string;fingerprint:string;status:number;body:string}|null>(null);
  const [busy,setBusy]=useState("");
  const [error,setError]=useState("");
  const patchExample=(exampleId:string,patch:Partial<RequestSpec["examples"][number]>)=>{
    setPreview(null);setError("");
    update({examples:request.examples.map(item=>item.id===exampleId?{...item,...patch}:item)});
  };
  let routePath="";
  try{
    routePath=new URL(request.url).pathname.replace(/%7B/gi,"{").replace(/%7D/gi,"}");
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
          <Editor value={example.body} readOnly dark={dark} height="150px" />
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
