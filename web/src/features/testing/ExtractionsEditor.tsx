import {Button,Card,Checkbox,Flex,Text,TextField} from "@radix-ui/themes";
import {Plus,Trash2} from "lucide-react";
import {Choice,ToolButton} from "../../shared/ui";
import {id} from "../../shared/model";
import {t,useLanguage} from "../../shared/i18n";
import type {Extraction,RequestSpec} from "../../shared/types";
export default function ExtractionsEditor({request,update}:{request:RequestSpec;update:(value:Partial<RequestSpec>)=>void}){
 useLanguage();const rules=request.extractions??[];const change=(rule:Extraction,patch:Partial<Extraction>)=>update({extractions:rules.map(value=>value.id===rule.id?{...value,...patch}:value)});
 return <Flex direction="column" gap="3"><Text weight="medium">{t("响应提取与变量设置")}</Text><Text size="1" color="gray">{t("提取在后置脚本之前执行，更新本次运行和本地变量覆盖，不自动修改共享工作区。环境变量写入当前选定环境。")}</Text>
 {rules.map(rule=><Card key={rule.id}><Flex gap="3" wrap="wrap" align="center">
  <label className="checkbox-label"><Checkbox checked={rule.enabled} onCheckedChange={value=>change(rule,{enabled:value===true})}/>{t("启用提取")}</label>
  <TextField.Root aria-label={t("提取名称")} value={rule.name} onChange={event=>change(rule,{name:event.target.value})}/>
  <Choice label={t("提取来源")} value={rule.kind} options={[{value:"json",label:"JSON Pointer"},{value:"jsonpath",label:"JSONPath"},{value:"xpath",label:"XPath"},{value:"regex",label:t("正则捕获")},{value:"header",label:"Header"},{value:"body",label:t("完整响应正文")}]} onChange={kind=>change(rule,{kind})}/>
  {rule.kind!=="body"&&<TextField.Root aria-label={t("提取路径 / 表达式")} placeholder={rule.kind==="json"?"/data/token":rule.kind==="jsonpath"?"$.data.token":rule.kind==="xpath"?"string(//*[local-name()='token'])":rule.kind==="header"?"X-Token":"token=(.+)"} value={rule.target} onChange={event=>change(rule,{target:event.target.value})}/>}
  <Choice label={t("目标变量范围")} value={rule.scope} options={[{value:"environment",label:t("环境变量")},{value:"collection",label:t("集合变量")},{value:"project",label:t("项目变量")},{value:"temporary",label:t("临时变量") }]} onChange={scope=>change(rule,{scope})}/>
  <TextField.Root aria-label={t("目标变量名称")} value={rule.key} onChange={event=>change(rule,{key:event.target.value})}/>
  <label className="checkbox-label"><Checkbox checked={rule.required} onCheckedChange={value=>change(rule,{required:value===true})}/>{t("必须提取成功")}</label>
  <ToolButton label={t("删除提取规则")} onClick={()=>update({extractions:rules.filter(value=>value.id!==rule.id)})}><Trash2 size={15}/></ToolButton>
 </Flex></Card>)}
 <Button variant="ghost" onClick={()=>update({extractions:[...rules,{id:id(),name:t("提取响应值"),kind:"json",target:"/data/token",scope:"environment",key:"token",enabled:true,required:true}]})}><Plus size={15}/>{t("添加响应提取")}</Button>
 </Flex>;
}
