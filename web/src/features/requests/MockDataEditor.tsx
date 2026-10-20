import { Button, Select, Text, TextField } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import { Field } from "../../shared/ui";
import type { RequestSpec, SavedDataset } from "../../shared/types";

type DataBinding=NonNullable<RequestSpec["examples"][number]["mock_data"]>;

export default function MockDataEditor({binding,datasets,onChange}:{binding:DataBinding;datasets:SavedDataset[];onChange:(binding:DataBinding)=>void}) {
  useLanguage();
  const parameters=binding.parameters??[];
  const dataset=datasets.find(item=>item.id===binding.dataset_id);
  const views=dataset?.views?.filter(view=>!!view.sql)??[];
  return <fieldset className="mock-data-editor"><legend>{t("数据驱动 Mock")}</legend>
    <Field label={t("保存的数据集")}><Select.Root value={dataset?.id??""} onValueChange={dataset_id=>{
      const next=datasets.find(item=>item.id===dataset_id);
      const view_id=next?.views?.find(view=>!!view.sql)?.id??"";
      onChange({...binding,dataset_id,view_id});
    }}><Select.Trigger aria-label={t("保存的数据集")} placeholder={t("数据集不可用")}/><Select.Content>{datasets.filter(item=>item.views?.some(view=>!!view.sql)).map(item=><Select.Item key={item.id} value={item.id}>{item.name}</Select.Item>)}</Select.Content></Select.Root></Field>
    <Field label={t("只读数据视图")}><Select.Root value={views.some(view=>view.id===binding.view_id)?binding.view_id:""} onValueChange={view_id=>onChange({...binding,view_id})}><Select.Trigger aria-label={t("只读数据视图")} placeholder={t("视图不可用")}/><Select.Content>{views.map(view=><Select.Item key={view.id} value={view.id}>{view.name}</Select.Item>)}</Select.Content></Select.Root></Field>
    {(!dataset||!views.some(view=>view.id===binding.view_id))&&<Text role="alert" size="1" color="red">{t("所选数据集或只读视图已不可用。")}</Text>}
    <Field label={t("数据响应形状")}><Select.Root value={binding.shape} onValueChange={shape=>onChange({...binding,shape:shape as DataBinding["shape"]})}><Select.Trigger aria-label={t("数据响应形状")}/><Select.Content><Select.Item value="rows">{t("所有匹配行")}</Select.Item><Select.Item value="first">{t("第一行")}</Select.Item></Select.Content></Select.Root></Field>
    <Text size="1" color="gray">{t("参数按顺序传入视图 SQL 的 ? 占位符；重复或敏感值会被拒绝。")}</Text>
    {parameters.map((parameter,index)=><div className="mock-data-parameter" key={index}>
      <Select.Root value={parameter.source} onValueChange={source=>onChange({...binding,parameters:parameters.map((item,position)=>position===index?{...item,source:source as NonNullable<DataBinding["parameters"]>[number]["source"]}:item)})}><Select.Trigger aria-label={`${t("参数来源")} ${index+1}`}/><Select.Content><Select.Item value="path">{t("路径变量")}</Select.Item><Select.Item value="query">{t("查询参数")}</Select.Item><Select.Item value="header">{t("请求头")}</Select.Item></Select.Content></Select.Root>
      <TextField.Root aria-label={`${t("参数名")} ${index+1}`} value={parameter.name} maxLength={128} onChange={event=>onChange({...binding,parameters:parameters.map((item,position)=>position===index?{...item,name:event.target.value}:item)})}/>
      <Button variant="soft" color="red" onClick={()=>onChange({...binding,parameters:parameters.filter((_,position)=>position!==index)})}>{t("移除参数")}</Button>
    </div>)}
    <Button variant="soft" disabled={parameters.length>=16} onClick={()=>onChange({...binding,parameters:[...parameters,{source:"query",name:""}]})}>{t("添加视图参数")}</Button>
  </fieldset>;
}
