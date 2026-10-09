import {useState} from "react";
import {Button,Callout,Flex,Text,TextField} from "@radix-ui/themes";
import {t} from "../../shared/i18n";
import {Choice,Field} from "../../shared/ui";
import {id} from "../../shared/model";
import type {Scenario,ScenarioParallel} from "../../shared/types";
export default function ScenarioParallelEditor({scenario,disabled,onChange}:{scenario:Scenario;disabled:boolean;onChange:(parallel:ScenarioParallel[])=>void}){
 const blocks=scenario.parallel??[],active=scenario.steps.filter(step=>step.enabled),used=new Set(blocks.flatMap(block=>block.step_ids));
 const [start,setStart]=useState(""),[count,setCount]=useState(2),[name,setName]=useState(""),[concurrency,setConcurrency]=useState(2),[invalid,setInvalid]=useState(false);
 function add(){const position=active.findIndex(step=>step.id===start),members=active.slice(position,position+count);if(position<0||!name.trim()||!Number.isInteger(count)||count<2||count>8||members.length!==count||members.some(step=>used.has(step.id)||step.on_true||step.on_false)){setInvalid(true);return;}onChange([...blocks,{id:id(),name:name.trim(),step_ids:members.map(step=>step.id),concurrency}]);setName("");setStart("");setInvalid(false);}
 return <Flex direction="column" gap="3"><Text weight="medium">{t("并行执行块")}</Text><Text size="1" color="gray">{t("选择连续的 2–8 个启用步骤，并发数为 1–4。分支变量相互隔离，汇合时不同写值会停止流程；成员支持条件和重复，不支持流程跳转。")}</Text>
 {blocks.map(block=><Flex key={block.id} gap="2" wrap="wrap" align="center"><Text>{block.name} · {block.step_ids.length}</Text><Field label={t("块并发数")}><Choice label={t("块并发数")} value={String(block.concurrency)} options={[1,2,3,4].map(value=>({value:String(value),label:String(value)}))} disabled={disabled} onChange={value=>onChange(blocks.map(item=>item.id===block.id?{...item,concurrency:Number(value)}:item))}/></Field><Button variant="ghost" disabled={disabled} onClick={()=>onChange(blocks.filter(item=>item.id!==block.id))}>{t("移除并行块")}</Button></Flex>)}
 <Flex gap="3" wrap="wrap" align="end"><Field label={t("并行块名称")}><TextField.Root value={name} disabled={disabled} maxLength={256} onChange={event=>setName(event.target.value)}/></Field><Choice label={t("并行起始步骤")} value={active.some(step=>step.id===start)?start:"none"} options={[{value:"none",label:t("选择起始步骤")},...active.filter(step=>!used.has(step.id)).map(step=>({value:step.id,label:`${scenario.steps.indexOf(step)+1} · ${step.name||step.request_id}`}))]} disabled={disabled} onChange={value=>setStart(value==="none"?"":value)}/><Field label={t("并行步骤数")}><TextField.Root type="number" min={2} max={8} value={count} disabled={disabled} onChange={event=>setCount(Number(event.target.value))}/></Field><Choice label={t("并发数")} value={String(concurrency)} options={[1,2,3,4].map(value=>({value:String(value),label:String(value)}))} disabled={disabled} onChange={value=>setConcurrency(Number(value))}/><Button variant="soft" disabled={disabled||!name.trim()||!start} onClick={add}>{t("添加并行块")}</Button></Flex>
 {invalid&&<Callout.Root color="red"><Callout.Text role="alert">{t("请选择连续、未分组且没有流程跳转的启用步骤，并检查数量和名称。")}</Callout.Text></Callout.Root>}
 </Flex>;
}
