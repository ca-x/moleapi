import {Flex,Text,TextArea,TextField} from "@radix-ui/themes";
import {Choice,Editor,Field} from "../../../shared/ui";
import {t,useLanguage} from "../../../shared/i18n";
import type {AsapAuth} from "../../../shared/types";
export const asapConfig=():AsapAuth=>({algorithm:"RS256",private_key:"",key_id:"",issuer:"",audience:[],subject:"",ttl_seconds:3600,claims_source:"{}"});
export default function AsapAuthEditor({config,change,dark}:{config:AsapAuth;change:(value:AsapAuth)=>void;dark:boolean}){
 useLanguage();const update=(patch:Partial<AsapAuth>)=>change({...config,...patch});
 return <Flex direction="column" gap="3">
 <Choice label={t("ASAP 签名算法")} value={config.algorithm} options={["RS256","RS384","RS512","PS256","PS384","PS512","ES256","ES384","ES512"].map(value=>({value,label:value}))} onChange={algorithm=>update({algorithm})}/>
 <Field label={t("ASAP 私钥")} hint={t("支持 PKCS8、RSA PKCS1、EC SEC1 PEM 和带 kid 的 PKCS8 data URI；不会读取文件路径或远程密钥。") }><TextArea autoComplete="off" className="mono" value={config.private_key} maxLength={65536} rows={4} onChange={e=>update({private_key:e.target.value})}/></Field>
 <Field label="Key ID / kid"><TextField.Root value={config.key_id} maxLength={4096} onChange={e=>update({key_id:e.target.value})}/></Field>
 <Field label="Issuer / iss"><TextField.Root value={config.issuer} maxLength={4096} onChange={e=>update({issuer:e.target.value})}/></Field>
 <Field label="Audience / aud" hint={t("多个 Audience 按顺序用英文逗号分隔，每项可使用变量。") }><TextField.Root value={config.audience.join(",")} onChange={e=>update({audience:e.target.value.split(',').map(s=>s.trim()).filter(Boolean)})}/></Field>
 <Field label={t("Subject / sub（可选）")} hint={t("留空时使用 Issuer。") }><TextField.Root value={config.subject} maxLength={4096} onChange={e=>update({subject:e.target.value})}/></Field>
 <Field label={t("ASAP 有效秒数")}><TextField.Root type="number" min={1} max={86400} value={config.ttl_seconds} onChange={e=>update({ttl_seconds:Number(e.target.value)})}/></Field>
 <Field label={t("ASAP Claims JSON")} hint={t("Claims 优先于独立字段；未提供时自动生成 jti、iat 和 exp。字符串变量按 JSON 类型安全解析。") }><Editor label={t("ASAP Claims JSON")} value={config.claims_source} onChange={claims_source=>update({claims_source})} dark={dark} jsonMode height="180px"/></Field>
 <Text size="1" color="gray">{t("每次执行生成新的 ASAP Bearer Token，支持变量和父级继承。默认导出和历史记录隐藏密钥与生成的 Token。")}</Text>
 </Flex>;
}
