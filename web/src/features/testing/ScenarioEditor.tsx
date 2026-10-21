import type { ReactNode } from "react";
import { Button, Card, Checkbox, Flex, Text, TextArea, TextField } from "@radix-ui/themes";
import { ArrowDown, ArrowUp, Plus, Trash2 } from "lucide-react";
import { t, useLanguage } from "../../shared/i18n";
import { Choice, Field, ToolButton } from "../../shared/ui";
import { id } from "../../shared/model";
import type { Scenario, ScenarioStep } from "../../shared/types";
import ScenarioParallelEditor from "./ScenarioParallelEditor";
import ScenarioStepControl from "./ScenarioStepControl";

export function scenarioDraftValid(value: Scenario) {
  return !!value.name.trim() && value.steps.length > 0 && value.steps.some((step) => step.enabled) && !value.steps.some((step) => !Number.isInteger(step.repeat ?? 1) || (step.repeat ?? 1) < 1 || (step.repeat ?? 1) > 1000);
}

export default function ScenarioEditor({ scenario: edit, requests, disabled: busy, notifications, onChange: setEdit }: { scenario: Scenario; requests: { value: string; label: string }[]; disabled: boolean; notifications?: ReactNode; onChange: (scenario: Scenario) => void }) {
  useLanguage();
 const change=(step:ScenarioStep,patch:Partial<ScenarioStep>)=>setEdit({...edit,steps:edit.steps.map(value=>{
  const updated=value.id===step.id?{...value,...patch}:value;
  if(patch.enabled===false)return endReferences(updated,step.id);
  return updated;
 })});
 function endReferences(step:ScenarioStep,targetId:string):ScenarioStep{return {...step,...(step.on_true?.action==="step"&&step.on_true.step_id===targetId?{on_true:{action:"stop" as const}}:{}),...(step.on_false?.action==="step"&&step.on_false.step_id===targetId?{on_false:{action:"stop" as const}}:{})};}
 function removeStep(targetId:string){setEdit({...edit,steps:edit.steps.filter(value=>value.id!==targetId).map(step=>endReferences(step,targetId))});}
 function move(index:number,offset:number){const steps=[...edit.steps];const destination=index+offset;if(destination<0||destination>=steps.length)return;[steps[index],steps[destination]]=[steps[destination],steps[index]];setEdit({...edit,steps});}
 const members=new Set((edit.parallel??[]).flatMap(block=>block.step_ids));
 const interiors=new Set((edit.parallel??[]).flatMap(block=>block.step_ids.slice(1)));
 const canMove=(index:number,offset:number)=>index+offset>=0&&index+offset<edit.steps.length&&!members.has(edit.steps[index].id)&&!members.has(edit.steps[index+offset].id);
 return <>
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
  {notifications}
 </>;
}
