import {useEffect,useRef,useState} from "react";
import {Button,Callout,Flex,Text} from "@radix-ui/themes";
import {t} from "../../shared/i18n";
import {id} from "../../shared/model";
import {useWorkbench} from "../workbench/context";
import NotificationPicker from "../notifications/NotificationPicker";
import ScenarioEditor, { scenarioDraftValid } from "./ScenarioEditor";
import type {Scenario} from "../../shared/types";
export default function SavedScenarios({collectionId,disabled}:{collectionId:string;disabled:boolean}){
 const state=useWorkbench();
 const fresh=():Scenario=>({id:"",name:"",description:"",collection_id:collectionId,steps:[]});
 const [edit,setEdit]=useState<Scenario>(fresh),[pending,setPending]=useState(false),[error,setError]=useState(false);
 const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
 if(!state.draft)return null;
 const scenarios=(state.draft.data.scenarios??[]).filter(scenario=>scenario.collection_id===collectionId);
 const collections=state.draft.data.collections;
 const included=new Set([collectionId]);let previous=0;
 while(previous!==included.size){previous=included.size;for(const collection of collections)if(collection.parent_id&&included.has(collection.parent_id))included.add(collection.id);}
 const requests=collections.filter(collection=>included.has(collection.id)).flatMap(collection=>(collection.requests??[]).map(request=>({value:request.id,label:`${collection.name} / ${request.name}`})));
 async function persist(removeId?:string){if(disabled||pending)return;setPending(true);setError(false);try{
  if(removeId){state.updateData(data=>({...data,scenarios:(data.scenarios??[]).filter(value=>value.id!==removeId)}));}
  else{
   const value={...edit,id:edit.id||id(),name:edit.name.trim()};
   if(!scenarioDraftValid(value))throw new Error("scenario input");
   state.updateData(data=>({...data,scenarios:[...(data.scenarios??[]).filter(scenario=>scenario.id!==value.id),value]}));
  }
  const saved=await state.save(true);if(mounted.current){if(saved)setEdit(fresh());else setError(true);}
 }catch{if(mounted.current)setError(true);}finally{if(mounted.current)setPending(false);}}
 const busy=disabled||pending;
 return <details><summary>{t("工作区测试场景")} ({scenarios.length})</summary><Flex direction="column" gap="3" mt="3">
  <Text size="1" color="gray">{t("场景保存请求引用、顺序和分组，随工作区同步。步骤使用原接口定义及当前环境，允许重复执行同一请求。")}</Text>
  {scenarios.map(scenario=><Flex key={scenario.id} gap="2" wrap="wrap" align="center"><Text>{scenario.name}</Text><Button variant="soft" disabled={busy} onClick={()=>{setEdit(structuredClone(scenario));setError(false);}}>{t("编辑场景")}</Button><Button variant="ghost" disabled={busy} onClick={()=>void persist(scenario.id)}>{t("删除场景")}</Button></Flex>)}
  <Button variant="soft" disabled={busy} onClick={()=>{setEdit(fresh());setError(false);}}>{t("新建场景")}</Button>
  <ScenarioEditor scenario={edit} requests={requests} disabled={busy} onChange={setEdit} notifications={<NotificationPicker label={t("场景默认通知对象")} value={edit.notification_ids??[]} disabled={busy} onChange={notification_ids=>setEdit({...edit,notification_ids})}/>} />
  <Button disabled={busy||!edit.name.trim()||!edit.steps.some(step=>step.enabled)} loading={pending} onClick={()=>void persist()}>{t("保存场景到工作区")}</Button>
  {error&&<Callout.Root color="red"><Callout.Text role="alert">{t("场景保存失败，请检查名称、请求引用、步骤数量或工作区版本冲突。未保存的编辑已保留。")}</Callout.Text></Callout.Root>}
 </Flex></details>;
}
