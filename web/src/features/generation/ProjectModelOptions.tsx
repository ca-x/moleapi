import {Checkbox,Flex,Text,TextField} from "@radix-ui/themes";
import {Choice,Field} from "../../shared/ui";
import {t} from "../../shared/i18n";
import type {ProjectTarget} from "./projectTypes";

export type ProjectOptions = Record<string,string|boolean>;
export default function ProjectModelOptions({target,options,disabled,onChange}:{target:ProjectTarget;options:ProjectOptions;disabled:boolean;onChange:(value:ProjectOptions)=>void}){
 const set=(name:string,value:string|boolean)=>onChange({...options,[name]:value});
 return <Flex direction="column" gap="2">
  <Field label={t("模型名称")}><TextField.Root disabled={disabled} maxLength={256} value={String(options.schemaName??"")} placeholder="*" onChange={event=>set("schemaName",event.target.value)}/></Field>
  <Text size="1" color="gray">{t("留空或 * 生成全部 components.schemas；填写名称只生成该模型及引用的类型。")}</Text>
  {!!target.model_options?.length&&<details><summary>{t("模型代码风格与序列化选项")}</summary><Flex gap="3" wrap="wrap" mt="2">
   {target.model_options.map(option=>{
    const value=options[option.name]??target.options?.[option.name];
    if(option.type==="boolean")return <label key={option.name} className="checkbox-label" title={option.description}><Checkbox disabled={disabled} checked={value===true} onCheckedChange={checked=>set(option.name,checked===true)}/>{option.name}</label>;
    return <Field key={option.name} label={option.name}>{option.type==="enum"?<Choice label={option.name} disabled={disabled} value={String(value??"")} options={(option.values??[]).map(value=>({value,label:value}))} onChange={value=>set(option.name,value)}/>:<TextField.Root disabled={disabled} maxLength={128} value={String(options[option.name]??"")} placeholder={String(target.options?.[option.name]??"")} title={option.description} onChange={event=>set(option.name,event.target.value)}/>}</Field>;
   })}
  </Flex></details>}
 </Flex>;
}
