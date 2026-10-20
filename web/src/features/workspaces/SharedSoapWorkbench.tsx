import { useState } from "react";
import { Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { ApiResponse, RequestSpec } from "../../shared/types";
import SoapClient from "../soap/SoapClient";
import SoapSourceEditor from "../soap/SoapSourceEditor";
import { useSharedSoapSchema, type SharedSoapSourceContext } from "./useSharedSoapSchema";
export default function SharedSoapWorkbench(props:SharedSoapSourceContext & {dark:boolean;onChange:(patch:Partial<RequestSpec>)=>void;response:ApiResponse|null;sending:boolean;onSend:()=>void;}) {
  useLanguage();const source=useSharedSoapSchema(props);const [open,setOpen]=useState(false);
  const unsavedDefinition=!!props.request.specification_id && !props.saved.data.specifications?.some(s=>s.id===props.request.specification_id&&s.kind==="wsdl"&&s.source===source.specification?.source);
  return <section className="shared-soap" aria-label={t("共享 SOAP 客户端")}>
    <Text as="p" size="1" color="gray">{t("定义与请求关联只在点击保存后写入共享工作区。")}</Text>
    {unsavedDefinition&&<Text as="p" size="1" color="gray">{t("先保存服务定义，再发送 SOAP 请求。")}</Text>}
    <SoapClient scope={props.scope} request={props.request} specifications={props.data.specifications??[]} schema={source.schema} dark={props.dark} onChange={props.onChange} isCurrent={props.isCurrent} disabled={props.saving||props.conflict} sourceError={source.error} sourceBusy={source.busy} sourceActionsDisabled={props.dirty||props.saving||props.conflict} onImport={()=>setOpen(true)} response={props.response} sending={props.sending} onSend={props.onSend} sendDisabled={props.saving||props.conflict||unsavedDefinition} sendLabel={props.dirty?t("发送草稿请求"):t("发送已保存请求")} />
    <SoapSourceEditor open={open} onOpenChange={setOpen} source={source} dark={props.dark} importSource={source.importSource}/>
  </section>;
}
