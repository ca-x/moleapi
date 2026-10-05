import {useEffect,useRef,useState} from "react";
import {Button,Checkbox,Flex,Text,TextField,Callout} from "@radix-ui/themes";
import {Field,Choice,Editor} from "../../shared/ui";
import {t,useLanguage,translateCopy} from "../../shared/i18n";
import {errorCopy,type ErrorCopy} from "../../shared/i18n/errors";
import {pickBinaryFile} from "../../shared/pickBinaryFile";
import {id} from "../../shared/model";
import {parseBody,emptyFile,type BodyFile,type BodyPart,type MultipartBody} from "./model";
export default function RequestBodyEditor({kind,value,change,busy,dark}:{kind:string;value:string;change:(source:string)=>void;busy:boolean;dark:boolean}) {
  useLanguage();const [inspect,setInspect]=useState(false),[pending,setPending]=useState(false),[error,setError]=useState<ErrorCopy>("");
  const latest=useRef({kind,value,change,busy});latest.current={kind,value,change,busy};
  const mounted=useRef(true),epoch=useRef(0),previousBusy=useRef(busy);
  if(busy&&!previousBusy.current)epoch.current++;previousBusy.current=busy;
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;epoch.current++;};},[]);
  let source:BodyFile|MultipartBody;
  try{source=parseBody(kind,value);}catch{return <><Callout.Root color="red"><Callout.Text>{t("请求体源数据有误，请修正 JSON 配置。")}</Callout.Text></Callout.Root><Editor value={value} onChange={change} dark={dark} jsonMode height="210px"/></>;}
  function commit(update:(body:BodyFile|MultipartBody)=>void){const current=latest.current;if(current.busy)return;try{const body=parseBody(current.kind,current.value);update(body);current.change(JSON.stringify(body));}catch(e){setError(errorCopy(e));}}
  async function select(partId?:string){const ticket=epoch.current;const origin=kind;const original=parseBody(origin,latest.current.value);const originalMime=origin==="binary"?(original as BodyFile).mime:(original as MultipartBody).parts.find(p=>p.id===partId)?.value;const mimeAtStart=typeof originalMime==="string"?originalMime:originalMime?.kind==="file"?originalMime.file.mime:"";setPending(true);setError("");try{const file=await pickBinaryFile();if(!file||!mounted.current||latest.current.busy||epoch.current!==ticket||latest.current.kind!==origin)return;
    commit(body=>{if(origin==="binary")Object.assign(body,{file_name:file.name,base64:file.base64,mime:(body as BodyFile).mime===mimeAtStart?file.mime||mimeAtStart:(body as BodyFile).mime});else{const part=(body as MultipartBody).parts.find(p=>p.id===partId);if(part?.value.kind==="file")part.value.file={...part.value.file,file_name:file.name,base64:file.base64,mime:part.value.file.mime===mimeAtStart?file.mime||mimeAtStart:part.value.file.mime};}});
  }catch(e){if(mounted.current)setError(errorCopy(e));}finally{if(mounted.current)setPending(false);}}
  function fileFields(file:BodyFile,partId?:string){return <Flex gap="2" wrap="wrap" align="end"><Button variant="soft" color="gray" disabled={busy||pending} loading={pending} onClick={()=>void select(partId)}>{t("选择上传文件")}</Button><Text size="2" className="wrap-anywhere">{file.file_name||t("尚未选择文件")}{file.base64===null?` · ${t("需要重新选择文件")}`:""}</Text><Field label={t("文件 MIME 类型")}><TextField.Root placeholder="application/octet-stream" value={file.mime} disabled={busy} onChange={e=>commit(body=>{if(kind==="binary")(body as BodyFile).mime=e.target.value;else{const part=(body as MultipartBody).parts.find(p=>p.id===partId);if(part?.value.kind==="file")part.value.file.mime=e.target.value;}})}/></Field></Flex>;}
  return <div className="request-file-body">
    <Text size="1" color="gray">{t("切换文件模式时，已有 Content-Type 会被禁用，原值保留在请求头中。")}</Text>
    <Text size="1" color="gray">{t("文件总量最多 5 MiB。默认导出不包含文件字节；上传响应仅保留在实时视图。")}</Text>
    {kind==="binary"?fileFields(source as BodyFile):<>
      <Text size="1" color="gray">{t("multipart 边界由工具生成，手动 Content-Type 需禁用。支持同名字段。")}</Text>
      {(source as MultipartBody).parts.map(part=><div className="multipart-part" key={part.id}>
        <Flex gap="2" wrap="wrap" align="end"><Checkbox aria-label={t("启用上传字段")} checked={part.enabled} disabled={busy} onCheckedChange={v=>commit(body=>{const p=(body as MultipartBody).parts.find(p=>p.id===part.id);if(p)p.enabled=v===true;})}/><Field label={t("字段名")}><TextField.Root value={part.name} disabled={busy} onChange={e=>commit(body=>{const p=(body as MultipartBody).parts.find(p=>p.id===part.id);if(p)p.name=e.target.value;})}/></Field><Choice label={t("字段类型")} value={part.value.kind} disabled={busy} options={[{value:"text",label:t("文本")},{value:"file",label:t("文件")}]} onChange={kind=>commit(body=>{const p=(body as MultipartBody).parts.find(p=>p.id===part.id);if(p)p.value=kind==="file"?{kind:"file",file:emptyFile()}:{kind:"text",text:"",mime:""};})}/><Button variant="ghost" color="gray" disabled={busy} onClick={()=>commit(body=>{(body as MultipartBody).parts=(body as MultipartBody).parts.filter(p=>p.id!==part.id);})}>{t("删除字段")}</Button></Flex>
        {part.value.kind==="file"?fileFields(part.value.file,part.id):<Field label={t("字段值")}><TextField.Root value={part.value.text} disabled={busy} onChange={e=>commit(body=>{const p=(body as MultipartBody).parts.find(p=>p.id===part.id);if(p?.value.kind==="text")p.value.text=e.target.value;})}/></Field>}
      </div>)}
      <Button variant="soft" color="gray" disabled={busy||(source as MultipartBody).parts.length>=64} onClick={()=>commit(body=>{(body as MultipartBody).parts.push({id:id(),name:"",enabled:true,value:{kind:"text",text:"",mime:""}} as BodyPart);})}>{t("添加上传字段")}</Button>
    </>}
    {error&&<Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
    <details onToggle={e=>setInspect(e.currentTarget.open)}><summary>{t("请求体源数据")}</summary>{inspect&&<Editor value={value} onChange={source=>{if(!latest.current.busy)change(source);}} dark={dark} jsonMode height="210px"/>}</details>
  </div>;
}
