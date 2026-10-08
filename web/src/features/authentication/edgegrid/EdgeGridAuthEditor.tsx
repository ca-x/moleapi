import {Flex,Text,TextField} from "@radix-ui/themes";
import {Field} from "../../../shared/ui";
import {t,useLanguage} from "../../../shared/i18n";
import type {EdgeGridAuth} from "../../../shared/types";
export const edgeGridConfig=():EdgeGridAuth=>({access_token:"",client_token:"",client_secret:"",base_url:"",headers_to_sign:[],nonce:"",timestamp:"",max_body_bytes:131072});
export default function EdgeGridAuthEditor({config,change}:{config:EdgeGridAuth;change:(value:EdgeGridAuth)=>void}){
 useLanguage();const update=(patch:Partial<EdgeGridAuth>)=>change({...config,...patch});
 return <Flex direction="column" gap="3">
 <Field label="Access Token"><TextField.Root type="password" autoComplete="off" maxLength={4096} value={config.access_token} onChange={e=>update({access_token:e.target.value})}/></Field>
 <Field label="Client Token"><TextField.Root type="password" autoComplete="off" maxLength={4096} value={config.client_token} onChange={e=>update({client_token:e.target.value})}/></Field>
 <Field label="Client Secret"><TextField.Root type="password" autoComplete="off" maxLength={65536} value={config.client_secret} onChange={e=>update({client_secret:e.target.value})}/></Field>
 <details><summary>{t("EdgeGrid 高级设置")}</summary><Flex direction="column" gap="3" mt="3">
 <Field label={t("EdgeGrid 签名 Base URL（可选）")} hint={t("留空使用实际请求 Host；覆盖只影响签名 Host，不修改实际请求地址。") }><TextField.Root value={config.base_url} maxLength={8192} onChange={e=>update({base_url:e.target.value})}/></Field>
 <Field label={t("参与签名的 Header 名称")} hint={t("按顺序使用英文逗号分隔；每个名称可引用环境变量。") }><TextField.Root value={config.headers_to_sign.join(",")} onChange={e=>update({headers_to_sign:e.target.value.split(',')})}/></Field>
 <Field label={t("EdgeGrid 最大哈希字节数")} hint={t("POST 只哈希这些前缀字节，完整请求体仍会发送；其他方法不哈希请求体。") }><TextField.Root type="number" min={1} max={5242880} value={config.max_body_bytes} onChange={e=>update({max_body_bytes:Number(e.target.value)})}/></Field>
 <Field label="Nonce" hint={t("留空时每次请求生成新的随机值。") }><TextField.Root value={config.nonce} maxLength={1024} onChange={e=>update({nonce:e.target.value})}/></Field>
 <Field label={t("EdgeGrid 时间戳（可选）")} hint={t("格式为 yyyyMMddTHH:mm:ss+0000；留空使用当前 UTC 时间。") }><TextField.Root value={config.timestamp} maxLength={128} onChange={e=>update({timestamp:e.target.value})}/></Field>
 </Flex></details>
 <Text size="1" color="gray">{t("使用 EG1-HMAC-SHA256 签名实际请求。凭据支持变量和继承；跨域重定向不会携带签名。")}</Text>
 </Flex>;
}
