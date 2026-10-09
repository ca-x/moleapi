import {useEffect,useState} from "react";
import {Button,Callout,Flex,Text,TextField} from "@radix-ui/themes";
import {Choice,Field} from "../../shared/ui";
import {t} from "../../shared/i18n";
import {templateKinds,type TemplateOutput} from "./templateBundle";
export default function TemplateOutputEditor({source,value,disabled,onApply}:{source:string;value:TemplateOutput|undefined;disabled:boolean;onApply:(value:TemplateOutput|undefined)=>void}){
 const override=source.endsWith(".mustache");
 const [kind,setKind]=useState(value?.templateType??(value?"SupportingFiles":override?"override":"SupportingFiles")),[folder,setFolder]=useState(value?.folder??""),[filename,setFilename]=useState(value?.destinationFilename??source.split("/").pop()!.replace(/\.mustache$/,"")),[error,setError]=useState(false);
 const saved=JSON.stringify(value);
 useEffect(()=>{setKind(value?.templateType??(value?"SupportingFiles":override?"override":"SupportingFiles"));setFolder(value?.folder??"");setFilename(value?.destinationFilename??source.split("/").pop()!.replace(/\.mustache$/,""));setError(false);},[source,saved]);
 function apply(){try{onApply(kind==="override"?undefined:{templateType:kind as typeof templateKinds[number],...(folder?{folder}:{}),...(filename?{destinationFilename:filename}:{})});setError(false);}catch{setError(true);}}
 return <Flex direction="column" gap="2">
  <Flex gap="3" wrap="wrap"><Field label={t("模板输出类型")}><Choice label={t("模板输出类型")} value={kind} disabled={disabled} options={[...(override?[{value:"override",label:t("仅覆盖上游模板")}]:[]),...templateKinds.map(value=>({value,label:value}))]} onChange={value=>{setKind(value);if(value!=="SupportingFiles")setFolder("");}}/></Field>
  {kind!=="override"&&<><Field label={t("输出文件名 / 后缀")}><TextField.Root maxLength={256} value={filename} disabled={disabled} onChange={event=>setFilename(event.target.value)}/></Field>{kind==="SupportingFiles"&&<Field label={t("额外文件输出目录")}><TextField.Root maxLength={256} value={folder} disabled={disabled} onChange={event=>setFolder(event.target.value)}/></Field>}</>}
  </Flex><Text size="1" color="gray">{t("API 和 Model 类型按接口或模型生成文件，名称字段是后缀；SupportingFiles 生成单个额外文件。应用映射后再生成。")}</Text>
  <Button variant="soft" disabled={disabled} onClick={apply}>{t("应用输出映射")}</Button>
  {error&&<Callout.Root color="red"><Callout.Text role="alert">{t("输出映射无效，请检查文件名、目录、重复目标和保留的清单路径。")}</Callout.Text></Callout.Root>}
 </Flex>;
}
