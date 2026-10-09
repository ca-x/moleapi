import {useEffect,useRef,useState} from "react";
import {Button,Callout,Card,Checkbox,Flex,Text,TextArea,TextField} from "@radix-ui/themes";
import {ArrowDown,ArrowUp,Plus,Trash2} from "lucide-react";
import {t} from "../../shared/i18n";
import {Choice,Field,ToolButton} from "../../shared/ui";
import {id} from "../../shared/model";
import {useWorkbench} from "../workbench/context";
import NotificationPicker from "../notifications/NotificationPicker";
import ScenarioParallelEditor from "./ScenarioParallelEditor";
import ScenarioStepControl from "./ScenarioStepControl";
import type {Scenario,ScenarioStep} from "../../shared/types";
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
 const change=(step:ScenarioStep,patch:Partial<ScenarioStep>)=>setEdit({...edit,steps:edit.steps.map(value=>{
  const updated=value.id===step.id?{...value,...patch}:value;
  if(patch.enabled===false)return endReferences(updated,step.id);
  return updated;
 })});
 function endReferences(step:ScenarioStep,targetId:string):ScenarioStep{return {...step,...(step.on_true?.action==="step"&&step.on_true.step_id===targetId?{on_true:{action:"stop" as const}}:{}),...(step.on_false?.action==="step"&&step.on_false.step_id===targetId?{on_false:{action:"stop" as const}}:{})};}
 function removeStep(targetId:string){setEdit({...edit,steps:edit.steps.filter(value=>value.id!==targetId).map(step=>endReferences(step,targetId))});}
 function move(index:number,offset:number){const steps=[...edit.steps];const destination=index+offset;if(destination<0||destination>=steps.length)return;[steps[index],steps[destination]]=[steps[destination],steps[index]];setEdit({...edit,steps});}
 async function persist(removeId?:string){if(disabled||pending)return;setPending(true);setError(false);try{
  if(removeId){state.updateData(data=>({...data,scenarios:(data.scenarios??[]).filter(value=>value.id!==removeId)}));}
  else{
   const value={...edit,id:edit.id||id(),name:edit.name.trim()};
   if(!value.name||!value.steps.length||!value.steps.some(step=>step.enabled)||value.steps.some(step=>!Number.isInteger(step.repeat??1)||(step.repeat??1)<1||(step.repeat??1)>1000))throw new Error("scenario input");
   state.updateData(data=>({...data,scenarios:[...(data.scenarios??[]).filter(scenario=>scenario.id!==value.id),value]}));
  }
  const saved=await state.save(true);if(mounted.current){if(saved)setEdit(fresh());else setError(true);}
 }catch{if(mounted.current)setError(true);}finally{if(mounted.current)setPending(false);}}
 const busy=disabled||pending;
 const members=new Set((edit.parallel??[]).flatMap(block=>block.step_ids));
 const interiors=new Set((edit.parallel??[]).flatMap(block=>block.step_ids.slice(1)));
 const canMove=(index:number,offset:number)=>index+offset>=0&&index+offset<edit.steps.length&&!members.has(edit.steps[index].id)&&!members.has(edit.steps[index+offset].id);
 return <details><summary>{t("工作区测试场景")} ({scenarios.length})</summary><Flex direction="column" gap="3" mt="3">
  <Text size="1" color="gray">{t("场景保存请求引用、顺序和分组，随工作区同步。步骤使用原接口定义及当前环境，允许重复执行同一请求。")}</Text>
  {scenarios.map(scenario=><Flex key={scenario.id} gap="2" wrap="wrap" align="center"><Text>{scenario.name}</Text><Button variant="soft" disabled={busy} onClick={()=>{setEdit(structuredClone(scenario));setError(false);}}>{t("编辑场景")}</Button><Button variant="ghost" disabled={busy} onClick={()=>void persist(scenario.id)}>{t("删除场景")}</Button></Flex>)}
  <Button variant="soft" disabled={busy} onClick={()=>{setEdit(fresh());setError(false);}}>{t("新建场景")}</Button>
  <Field label={t("场景名称")}><TextField.Root disabled={busy} value={edit.name} maxLength={256} onChange={event=>setEdit({...edit,name:event.target.value})}/></Field>
  <Field label={t("场景说明")}><TextArea disabled={busy} value={edit.description} maxLength={4096} onChange={event=>setEdit({...edit,description:event.target.value})}/></Field>
  {edit.steps.map((step,index)=><Card key={step.id}><Flex gap="2" wrap="wrap" align="center">
   <Text size="1">{index+1}</Text><label className="checkbox-label"><Checkbox checked={step.enabled} disabled={busy||members.has(step.id)} onCheckedChange={enabled=>change(step,{enabled:enabled===true})}/>{t("启用步骤")}</label>
   <Choice label={t("场景请求")} disabled={busy} value={step.request_id} options={requests} onChange={request_id=>change(step,{request_id})}/>
   <TextField.Root aria-label={t("步骤名称")} disabled={busy} value={step.name} maxLength={256} onChange={event=>change(step,{name:event.target.value})}/>
   <TextField.Root aria-label={t("步骤分组")} placeholder={t("步骤分组")} disabled={busy} value={step.group} maxLength={256} onChange={event=>change(step,{group:event.target.value})}/>
   <ToolButton label={t("上移步骤")} disabled={busy||!canMove(index,-1)} onClick={()=>move(index,-1)}><ArrowUp size={15}/></ToolButton>
   <ToolButton label={t("下移步骤")} disabled={busy||!canMove(index,1)} onClick={()=>move(index,1)}><ArrowDown size={15}/></ToolButton>
   <ToolButton label={t("删除步骤")} disabled={busy||members.has(step.id)} onClick={()=>removeStep(step.id)}><Trash2 size={15}/></ToolButton>
  </Flex>{members.has(step.id)&&<Text size="1" color="gray">{t("并行块成员：请先移除并行块，再删除、禁用或调整步骤顺序。")}</Text>}<ScenarioStepControl step={step} steps={edit.steps} parallel={members.has(step.id)} interiors={interiors} disabled={busy} onChange={patch=>change(step,patch)}/></Card>)}
  <Text size="1" color="gray">{t("删除或禁用目标步骤会将指向它的分支改为结束本轮。脚本跳转优先于场景重复和分支设置。")}</Text>
  <Button variant="soft" disabled={busy||!requests.length||edit.steps.length>=1000} onClick={()=>setEdit({...edit,steps:[...edit.steps,{id:id(),request_id:requests[0].value,name:"",group:"",enabled:true}]})}><Plus size={15}/>{t("添加请求步骤")}</Button>
  <ScenarioParallelEditor scenario={edit} disabled={busy} onChange={parallel=>setEdit({...edit,parallel})}/>
  <NotificationPicker label={t("场景默认通知对象")} value={edit.notification_ids??[]} disabled={busy} onChange={notification_ids=>setEdit({...edit,notification_ids})}/>
  <Button disabled={busy||!edit.name.trim()||!edit.steps.some(step=>step.enabled)} loading={pending} onClick={()=>void persist()}>{t("保存场景到工作区")}</Button>
  {error&&<Callout.Root color="red"><Callout.Text role="alert">{t("场景保存失败，请检查名称、请求引用、步骤数量或工作区版本冲突。未保存的编辑已保留。")}</Callout.Text></Callout.Root>}
 </Flex></details>;
}
