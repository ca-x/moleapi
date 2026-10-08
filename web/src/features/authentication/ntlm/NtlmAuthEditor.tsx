import {Checkbox,Flex,Text,TextField} from "@radix-ui/themes";
import {Field} from "../../../shared/ui";
import {t,useLanguage} from "../../../shared/i18n";
import type {NtlmAuth} from "../../../shared/types";
export const ntlmConfig=():NtlmAuth=>({domain:"",workstation:"",channel_binding:true});
export default function NtlmAuthEditor({config,change}:{config:NtlmAuth;change:(value:NtlmAuth)=>void}){
 useLanguage();const update=(patch:Partial<NtlmAuth>)=>change({...config,...patch});
 return <Flex direction="column" gap="3"><Field label={t("NTLM 域（可选）")} hint={t("域留空时可在用户名中使用 DOMAIN\\user 或 user@domain。") }><TextField.Root value={config.domain} maxLength={256} onChange={e=>update({domain:e.target.value})}/></Field><Field label={t("NTLM 工作站（可选）")}><TextField.Root value={config.workstation} maxLength={256} onChange={e=>update({workstation:e.target.value})}/></Field><Text as="label" size="2"><Flex gap="2"><Checkbox checked={config.channel_binding} onCheckedChange={v=>update({channel_binding:v===true})}/>{t("HTTPS 使用 TLS 通道绑定")}</Flex></Text><Text size="1" color="gray">{t("使用 NTLMv2，握手保持在同一条 HTTP/1.1 连接中。不会读取系统登录凭据；断连时停止认证，不向跨域重定向发送凭据。")}</Text></Flex>;
}
