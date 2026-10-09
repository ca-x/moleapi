import {Flex,Text,TextArea,TextField} from "@radix-ui/themes";
import {t} from "../../shared/i18n";
import {Choice,Field} from "../../shared/ui";
import type {ScenarioStep,ScenarioTarget} from "../../shared/types";
const encode=(value?:ScenarioTarget)=>!value?"next":value.action==="stop"?"stop":`step:${value.step_id}`;
const decode=(value:string):ScenarioTarget|undefined=>value==="next"?undefined:value==="stop"?{action:"stop"}:{action:"step",step_id:value.slice(5)};
export default function ScenarioStepControl({step,steps,disabled,onChange,parallel=false,interiors=new Set<string>()}:{step:ScenarioStep;steps:ScenarioStep[];disabled:boolean;parallel?:boolean;interiors?:Set<string>;onChange:(patch:Partial<ScenarioStep>)=>void}){
 const targets=[{value:"next",label:t("下一启用步骤")},{value:"stop",label:t("结束本轮场景")},...steps.flatMap((candidate,index)=>candidate.enabled&&!interiors.has(candidate.id)?[{value:`step:${candidate.id}`,label:`${index+1} · ${candidate.name||candidate.request_id}`}]:[])];
 return <Flex direction="column" gap="2" mt="3"><Field label={t("执行条件（JavaScript 布尔表达式）")}><TextArea disabled={disabled} value={step.condition??""} maxLength={4096} placeholder="pm.variables.get('ready') === 'yes'" onChange={event=>onChange({condition:event.target.value.trim()?event.target.value:undefined})}/></Field>
 <Text size="1" color="gray">{t("条件在请求脚本之前读取变量、数据和前一次响应。条件内的修改不会应用；空条件总是成立。")}</Text>
 <Flex gap="3" wrap="wrap"><Field label={t("重复次数")}><TextField.Root type="number" min={1} max={1000} disabled={disabled} value={step.repeat??1} onChange={event=>onChange({repeat:Number(event.target.value)})}/></Field>
 <Choice label={t("条件成立后")} disabled={disabled||parallel} value={encode(step.on_true)} options={targets} onChange={value=>onChange({on_true:decode(value)})}/>
 <Choice label={t("条件不成立后")} disabled={disabled||parallel||!step.condition} value={encode(step.on_false)} options={targets} onChange={value=>onChange({on_false:decode(value)})}/></Flex>
 </Flex>;
}
