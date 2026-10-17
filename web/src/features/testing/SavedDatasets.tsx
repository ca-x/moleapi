import {useEffect,useRef,useState} from "react";
import {Button,Callout,Checkbox,Flex,Text,TextArea,TextField} from "@radix-ui/themes";
import {api} from "../../shared/api";
import {t} from "../../shared/i18n";
import {Field} from "../../shared/ui";
import {id} from "../../shared/model";
import {useWorkbench} from "../workbench/context";
import type {SavedDataset,SavedDatasetView} from "../../shared/types";
import RunnerDataset from "./RunnerDataset";
import SavedDatasetViews from "./SavedDatasetViews";
const empty=()=>({id:"",name:"",table_name:"data",description:"",secret:true,format:"json" as "csv"|"json",source:"",views:[] as SavedDatasetView[]});
export default function SavedDatasets({scope,disabled}:{scope:string;disabled:boolean}){
 const state=useWorkbench(),[edit,setEdit]=useState(empty),[pending,setPending]=useState(false),[error,setError]=useState(false),[previewBusy,setPreviewBusy]=useState(false);
 const origin=JSON.stringify([scope,edit]);const latest=useRef({origin,state});latest.current={origin,state};const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
 if(!state.draft)return null;const datasets=state.draft.data.datasets??[];
 function choose(dataset:SavedDataset){setEdit({id:dataset.id,name:dataset.name,table_name:dataset.table_name??"data",description:dataset.description,secret:dataset.secret,format:dataset.source?.format??"json",source:dataset.source?.source??"",views:dataset.views??[]});setError(false);}
 async function save(){if(pending||previewBusy||!state.draft)return;const identity=origin,workspaceId=state.draft.id;const current=()=>mounted.current&&latest.current.origin===identity&&latest.current.state.draft?.id===workspaceId;
  setPending(true);setError(false);try{
   if(!edit.name.trim()||new TextEncoder().encode(edit.name).length>256||new TextEncoder().encode(edit.description).length>4096||!/^[A-Za-z_][A-Za-z0-9_]{0,63}$/.test(edit.table_name)||(!edit.id&&datasets.length>=20)||edit.views.length>20||new Set(edit.views.map(view=>view.id)).size!==edit.views.length||edit.views.some(view=>!view.name.trim()||new TextEncoder().encode(view.name).length>256||view.sql!==undefined&&(!view.sql.trim()||new TextEncoder().encode(view.sql).length>65536)))throw new Error("dataset metadata");
   const source=edit.source.trim()?{format:edit.format,source:edit.source}:undefined;if(source)await api("/api/testing/dataset/preview","POST",{workspace_id:workspaceId,dataset:source});if(!current())return;
   const value:SavedDataset={id:edit.id||id(),name:edit.name.trim(),table_name:edit.table_name,description:edit.description,secret:edit.secret,views:edit.views,...(source?{source}:{})};
   state.updateData(data=>({...data,datasets:[...(data.datasets??[]).filter(dataset=>dataset.id!==value.id),value]}));const saved=await state.save(true);if(!saved&&current())setError(true);else if(current())setEdit(empty());
  }catch{if(current())setError(true);}finally{if(mounted.current)setPending(false);}
 }
 async function remove(datasetId:string){if(pending||!state.draft)return;const identity=origin;setPending(true);setError(false);try{state.updateData(data=>({...data,datasets:(data.datasets??[]).filter(dataset=>dataset.id!==datasetId)}));const saved=await state.save(true);if(mounted.current&&latest.current.origin===identity){if(!saved)setError(true);else if(edit.id===datasetId)setEdit(empty());}}catch{if(mounted.current&&latest.current.origin===identity)setError(true);}finally{if(mounted.current)setPending(false);}}
 return <details><summary>{t("工作区数据集")} ({datasets.length})</summary><Flex direction="column" gap="3" mt="3">
  <Text size="1" color="gray">{t("数据集随工作区保存和同步。默认备份隐藏私密数据源；包含私密值的备份保留原文。")}</Text>
  {datasets.map(dataset=><Flex key={dataset.id} gap="2" align="center" wrap="wrap"><Text>{dataset.name}</Text>{!dataset.source&&<Text size="1" color="gray">{t("数据源缺失，请重新导入。")}</Text>}<Button variant="soft" disabled={disabled||pending} onClick={()=>choose(dataset)}>{t("编辑数据集")}</Button><Button variant="ghost" disabled={disabled||pending} onClick={()=>void remove(dataset.id)}>{t("删除数据集")}</Button></Flex>)}
  <Button variant="soft" disabled={disabled||pending} onClick={()=>{setEdit(empty());setError(false);}}>{t("新建数据集")}</Button>
  <Field label={t("数据集名称")}><TextField.Root value={edit.name} disabled={disabled||pending} maxLength={256} onChange={event=>setEdit({...edit,name:event.target.value})}/></Field>
  {edit.id&&<Field label={t("脚本数据集 ID")} hint={t("在脚本中使用 pm.datasets(ID) 引用此数据集。点击字段即可选中完整 ID。") }><TextField.Root readOnly value={edit.id} onFocus={event=>event.currentTarget.select()}/></Field>}
  <Field label={t("SQL 表名")} hint={t("脚本查询此数据集时使用；以字母或下划线开头，只能包含英文字母、数字和下划线。") }><TextField.Root value={edit.table_name} disabled={disabled||pending} maxLength={64} onChange={event=>setEdit({...edit,table_name:event.target.value})}/></Field>
  <Field label={t("数据集说明")}><TextArea value={edit.description} disabled={disabled||pending} maxLength={4096} onChange={event=>setEdit({...edit,description:event.target.value})}/></Field>
  <SavedDatasetViews key={`${scope}:${edit.id}`} views={edit.views} onChange={views=>setEdit({...edit,views})} disabled={disabled||pending}/>
  <label className="checkbox-label"><Checkbox checked={edit.secret} disabled={disabled||pending} onCheckedChange={secret=>setEdit({...edit,secret:secret===true})}/>{t("私密数据集（默认备份不包含数据源）")}</label>
  <RunnerDataset workspaceId={state.draft.id} scope={scope} value={{format:edit.format,source:edit.source,iterations:""}} disabled={disabled||pending} dark={state.dark} onBusyChange={setPreviewBusy} onChange={value=>setEdit({...edit,format:value.format,source:value.source})}/>
  <Button disabled={disabled||pending||previewBusy||!edit.name.trim()} loading={pending} onClick={()=>void save()}>{t("保存数据集到工作区")}</Button>
  {error&&<Callout.Root color="red"><Callout.Text role="alert">{t("数据集保存失败，请检查数据、名称、大小或工作区版本冲突。未保存的编辑已保留。")}</Callout.Text></Callout.Root>}
 </Flex></details>;
}
