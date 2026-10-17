import { useState } from "react";
import { t, useLanguage } from "../../shared/i18n";
import { Badge, Button, Card, Flex, Text, TextArea, TextField } from "@radix-ui/themes";
import { Trash2 } from "lucide-react";
import { previewMock, token } from "../../shared/api";
import { Editor, Field, ToolButton } from "../../shared/ui";
import type { RequestSpec } from "../../shared/types";
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
          {workspaceId && <Button variant="soft" disabled={dirty||busy!==""} loading={busy===`${workspaceId}/${request.id}/${example.id}`} onClick={()=>previewExample(example.id)}>{t("预览 Mock")}</Button>}
          {dirty && <Text size="1" color="gray">{t("先保存工作区，再预览 Mock。")}</Text>}
          {error===`${workspaceId}/${request.id}/${example.id}` && <Text role="alert" size="1" color="red">{t("Mock 预览失败，请检查脚本与数据源。")}</Text>}
          {!dirty && preview?.scope===`${workspaceId}/${request.id}/${example.id}` && preview.fingerprint===JSON.stringify(example) && <Field label={t("Mock 预览结果")} hint={`${t("状态码")}: ${preview.status}`}><TextArea readOnly rows={5} value={preview.body.slice(0,64*1024)} /></Field>}
          {asyncMock ? <Field label={t("Mock 实现脚本")} hint={t("使用 pm.mock 设置状态、响应头与正文；可通过 pm.datasets(ID) 查询工作区数据集。") }><Editor label={t("Mock 实现脚本")} value={example.mock_script??""} onChange={mock_script=>{setPreview(null);setError("");update({examples:request.examples.map(item=>item.id===example.id?{...item,mock_script}:item)});}} dark={dark} language="javascript" height="180px"/></Field> : <Text size="1" color="gray">{t("在工作区设置中启用 v2 脚本模式后可编辑动态 Mock。")}</Text>}
        </Card>
      ))}
    </div>
  );
}
