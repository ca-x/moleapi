import {useEffect,useRef,useState} from "react";
import {Button,Callout,Flex,Text} from "@radix-ui/themes";
import {api} from "../../shared/api";
import {t} from "../../shared/i18n";
import {useWorkbench} from "../workbench/context";
import type {SavedDataset} from "../../shared/types";
import RunnerDataset from "./RunnerDataset";
import DatasetFields from "./DatasetFields";
import {datasetEdit as empty,datasetFromEdit,validDatasetEdit,type DatasetEdit as Edit} from "./datasetEdit";
export default function SavedDatasets({scope,disabled}:{scope:string;disabled:boolean}){
 const state=useWorkbench(),[edit,setEdit]=useState<Edit>(()=>empty()),[pending,setPending]=useState(false),[error,setError]=useState(false),[previewBusy,setPreviewBusy]=useState(false);
 const origin=JSON.stringify([scope,edit]);const latest=useRef({origin,state});latest.current={origin,state};const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
 if(!state.draft)return null;const datasets=state.draft.data.datasets??[];
 const requests=(state.draft.data.collections??[]).flatMap(collection=>collection.requests.filter(request=>request.protocol?.kind==="data"&&request.protocol.read_only&&request.protocol.sql.trim()).map(request=>({value:request.id,label:`${collection.name} / ${request.name}`})));
 function choose(dataset:SavedDataset){setEdit(empty(dataset));setError(false);}
 async function save(){if(pending||previewBusy||!state.draft)return;const identity=origin,workspaceId=state.draft.id;const current=()=>mounted.current&&latest.current.origin===identity&&latest.current.state.draft?.id===workspaceId;
  setPending(true);setError(false);try{
   if(!validDatasetEdit(edit,datasets,requests))throw new Error("dataset metadata");
   const source=edit.mode==="literal"&&edit.source.trim()?{format:edit.format,source:edit.source}:undefined;
   if(edit.mode==="query"&&!requests.some(request=>request.value===edit.requestId))throw new Error("missing Data request");
   if(source)await api("/api/testing/dataset/preview","POST",{workspace_id:workspaceId,dataset:source});if(!current())return;
   const value:SavedDataset=datasetFromEdit(edit);
   state.updateData(data=>({...data,datasets:[...(data.datasets??[]).filter(dataset=>dataset.id!==value.id),value]}));const saved=await state.save(true);if(!saved&&current())setError(true);else if(current())setEdit(empty());
  }catch{if(current())setError(true);}finally{if(mounted.current)setPending(false);}
 }
 async function remove(datasetId:string){if(pending||!state.draft)return;const identity=origin;setPending(true);setError(false);try{state.updateData(data=>({...data,datasets:(data.datasets??[]).filter(dataset=>dataset.id!==datasetId)}));const saved=await state.save(true);if(mounted.current&&latest.current.origin===identity){if(!saved)setError(true);else if(edit.id===datasetId)setEdit(empty());}}catch{if(mounted.current&&latest.current.origin===identity)setError(true);}finally{if(mounted.current)setPending(false);}}
 return <details><summary>{t("工作区数据集")} ({datasets.length})</summary><Flex direction="column" gap="3" mt="3">
  <Text size="1" color="gray">{t("数据集随工作区保存和同步。默认备份隐藏私密数据源；包含私密值的备份保留原文。")}</Text>
  {datasets.map(dataset=><Flex key={dataset.id} gap="2" align="center" wrap="wrap"><Text>{dataset.name}</Text>{dataset.query&&<Text size="1" color="gray">{t("Data 查询（每次运行刷新）")}</Text>}{!dataset.source&&!dataset.query&&<Text size="1" color="gray">{t("数据源缺失，请重新导入。")}</Text>}<Button variant="soft" color="gray" disabled={disabled||pending} onClick={()=>choose(dataset)}>{t("编辑数据集")}</Button><Button variant="ghost" disabled={disabled||pending} onClick={()=>void remove(dataset.id)}>{t("删除数据集")}</Button></Flex>)}
  <Button variant="soft" color="gray" disabled={disabled||pending} onClick={()=>{setEdit(empty());setError(false);}}>{t("新建数据集")}</Button>
  <DatasetFields edit={edit} requests={requests} scope={scope} disabled={disabled||pending} onChange={setEdit} literalSource={<RunnerDataset workspaceId={state.draft.id} scope={scope} value={{format:edit.format,source:edit.source,iterations:""}} disabled={disabled||pending} dark={state.dark} onBusyChange={setPreviewBusy} onChange={value=>setEdit({...edit,format:value.format,source:value.source})}/>} />
  <Button disabled={disabled||pending||previewBusy||!edit.name.trim()||(edit.mode==="query"&&!requests.some(request=>request.value===edit.requestId))} loading={pending} onClick={()=>void save()}>{t("保存数据集到工作区")}</Button>
  {error&&<Callout.Root color="red"><Callout.Text role="alert">{t("数据集保存失败，请检查数据、名称、大小或工作区版本冲突。未保存的编辑已保留。")}</Callout.Text></Callout.Root>}
 </Flex></details>;
}
