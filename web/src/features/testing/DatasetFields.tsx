import type { ReactNode } from "react";
import { Checkbox, Text, TextArea, TextField } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import { Choice, Field } from "../../shared/ui";
import SavedDatasetViews from "./SavedDatasetViews";
import type { DatasetEdit } from "./datasetEdit";

export default function DatasetFields({ edit, requests, scope, disabled, literalSource, onChange: setEdit }: { edit: DatasetEdit; requests: { value: string; label: string }[]; scope: string; disabled: boolean; literalSource: ReactNode; onChange: (edit: DatasetEdit) => void }) {
  useLanguage();
  return <>
  <Field label={t("数据集名称")}><TextField.Root value={edit.name} disabled={disabled} maxLength={256} onChange={event=>setEdit({...edit,name:event.target.value})}/></Field>
  {edit.id&&<Field label={t("脚本数据集 ID")} hint={t("在脚本中使用 pm.datasets(ID) 引用此数据集。点击字段即可选中完整 ID。") }><TextField.Root readOnly value={edit.id} onFocus={event=>event.currentTarget.select()}/></Field>}
  <Field label={t("SQL 表名")} hint={t("脚本查询此数据集时使用；以字母或下划线开头，只能包含英文字母、数字和下划线。") }><TextField.Root value={edit.table_name} disabled={disabled} maxLength={64} onChange={event=>setEdit({...edit,table_name:event.target.value})}/></Field>
  <Field label={t("数据集说明")}><TextArea value={edit.description} disabled={disabled} maxLength={4096} onChange={event=>setEdit({...edit,description:event.target.value})}/></Field>
  <SavedDatasetViews key={`${scope}:${edit.id}`} views={edit.views} onChange={views=>setEdit({...edit,views})} disabled={disabled}/>
  <Field label={t("数据集来源")}><Choice<"literal"|"query"> label={t("数据集来源")} value={edit.mode} disabled={disabled} options={[{value:"literal",label:t("CSV / JSON 文本")},{value:"query",label:t("Data 查询（每次运行刷新）") }]} onChange={mode=>setEdit({...edit,mode,secret:mode==="query"?true:edit.secret})}/></Field>
  {edit.mode==="query"?<><Field label={t("选择只读 Data 请求")}><Choice label={t("选择只读 Data 请求")} value={edit.requestId||"none"} disabled={disabled||requests.length===0} options={[{value:"none",label:t("选择只读 Data 请求")},...requests]} onChange={requestId=>setEdit({...edit,requestId:requestId==="none"?"":requestId})}/></Field><Text size="1" color="gray">{t("每次运行前重新查询全部数据行；查询失败时不会执行集合请求。查询来源始终为私密。")}</Text>{requests.length===0&&<Text size="1" color="red">{t("先保存一个只读 Data 请求。")}</Text>}</>:<><label className="checkbox-label"><Checkbox checked={edit.secret} disabled={disabled} onCheckedChange={secret=>setEdit({...edit,secret:secret===true})}/>{t("私密数据集（默认备份不包含数据源）")}</label>{literalSource}</>}
  </>;
}
