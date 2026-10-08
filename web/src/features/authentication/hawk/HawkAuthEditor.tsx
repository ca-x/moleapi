import {Checkbox,Flex,Text,TextField} from "@radix-ui/themes";
import {Choice,Field} from "../../../shared/ui";
import {t,useLanguage} from "../../../shared/i18n";
import type {HawkAuth} from "../../../shared/types";
export const hawkConfig=():HawkAuth=>({id:"",key:"",algorithm:"sha256",nonce:"",timestamp:"",ext:"",app:"",delegation:"",user:"",include_payload_hash:false});
export default function HawkAuthEditor({config,change}:{config:HawkAuth;change:(value:HawkAuth)=>void}) {
 useLanguage();const update=(patch:Partial<HawkAuth>)=>change({...config,...patch});
 const text=(name:"id"|"nonce"|"timestamp"|"ext"|"app"|"delegation"|"user",label:string,limit:number,hint?:string)=><Field label={label} hint={hint}><TextField.Root value={config[name]} maxLength={limit} autoComplete="off" onChange={e=>update({[name]:e.target.value})}/></Field>;
 return <Flex direction="column" gap="3">
 {text("id",t("Hawk 凭据 ID"),4096)}
 <Field label={t("Hawk 密钥")}><TextField.Root type="password" value={config.key} autoComplete="off" maxLength={65536} onChange={e=>update({key:e.target.value})}/></Field>
 <Choice label={t("Hawk 算法")} value={config.algorithm} options={[{value:"sha256",label:"SHA256"},{value:"sha1",label:"SHA1"},{value:"sha384",label:"SHA384"},{value:"sha512",label:"SHA512"}]} onChange={algorithm=>update({algorithm})}/>
 <Text as="label" size="2"><Flex gap="2"><Checkbox checked={config.include_payload_hash} onCheckedChange={value=>update({include_payload_hash:value===true})}/>{t("签名包含实际请求体哈希")}</Flex></Text>
 <details><summary>{t("Hawk 高级参数")}</summary><Flex direction="column" gap="3" mt="3">
 {text("nonce","Nonce",1024,t("留空时自动生成随机值，可引用环境变量。"))}
 {text("timestamp",t("Hawk 时间戳（Unix 秒）"),128,t("留空时使用发送时间，可引用环境变量。"))}
 {text("ext",t("Hawk 附加数据"),4096)}{text("app",t("Hawk 应用 ID"),1024)}{text("delegation",t("Hawk 委托 ID"),1024)}
 </Flex></details>
 <Text size="1" color="gray">{t("应用与委托参与签名。启用请求体哈希时，按实际发送的内容和 MIME 类型计算。")}</Text>
 </Flex>;
}
