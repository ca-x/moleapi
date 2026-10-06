import {useEffect,useState} from "react";
import TokenManager from "./TokenManager";
import {Checkbox,Flex,Text,TextField} from "@radix-ui/themes";
import {Choice,Field,PairEditor} from "../../../shared/ui";
import {t,useLanguage} from "../../../shared/i18n";
import type {OAuth2Auth} from "./types";
export default function OAuth2ConfigEditor({config,change,query=true}:{config:OAuth2Auth;change:(value:OAuth2Auth)=>void;query?:boolean}) {
 useLanguage();const [scopeText,setScopeText]=useState(config.scopes.join(" "));const scopesKey=config.scopes.join(" ");useEffect(()=>setScopeText(scopesKey),[scopesKey]);const update=(patch:Partial<OAuth2Auth>)=>change({...config,...patch});
 const url=(key:"authorization_url"|"token_url"|"device_url"|"redirect_url"|"revocation_url"|"introspection_url",label:string)=><Field label={label}><TextField.Root value={config[key]} maxLength={8192} onChange={e=>update({[key]:e.target.value})}/></Field>;
 return <Flex direction="column" gap="3">
  <Choice label={t("OAuth2 授权方式")} value={config.grant} options={[{value:"authorization_code",label:"Authorization Code"},{value:"implicit",label:"Implicit"},{value:"client_credentials",label:"Client Credentials"},{value:"password",label:"Password"},{value:"device_code",label:"Device Code"}]} onChange={grant=>update({grant})}/>
  {["authorization_code","implicit"].includes(config.grant)&&<>{url("authorization_url",t("授权 URL"))}{url("redirect_url",t("回调 URI"))}</>}
  {config.grant!=="implicit"&&url("token_url",t("Token URL"))}
  {config.grant==="device_code"&&url("device_url",t("设备授权 URL"))}
  <Field label="Client ID"><TextField.Root value={config.client_id} maxLength={4096} onChange={e=>update({client_id:e.target.value})}/></Field>
  {config.grant!=="implicit"&&<Field label="Client Secret"><TextField.Root type="password" autoComplete="off" value={config.client_secret} maxLength={65536} onChange={e=>update({client_secret:e.target.value})}/></Field>}
  {config.grant==="password"&&<><Field label={t("用户名")}><TextField.Root autoComplete="off" value={config.username} onChange={e=>update({username:e.target.value})}/></Field><Field label={t("密码")}><TextField.Root type="password" autoComplete="off" value={config.password} onChange={e=>update({password:e.target.value})}/></Field></>}
  <Field label="Scopes" hint={t("权限范围以空格分隔，可引用环境变量。") }><TextField.Root value={scopeText} onChange={e=>{setScopeText(e.target.value);update({scopes:e.target.value.split(/\s+/).filter(Boolean)});}}/></Field>
  <Choice label={t("客户端凭据位置")} value={config.client_auth} options={[{value:"basic",label:"Basic Auth header"},{value:"body",label:"Request body"}]} onChange={client_auth=>update({client_auth})}/>
  {config.grant==="authorization_code"&&<Text as="label" size="2"><Flex gap="2"><Checkbox checked={config.pkce} onCheckedChange={value=>update({pkce:value===true})}/>{t("使用 SHA256 PKCE")}</Flex></Text>}
  <Text as="label" size="2"><Flex gap="2"><Checkbox checked={config.auto_refresh} onCheckedChange={value=>update({auto_refresh:value===true})}/>{t("执行时自动刷新过期 Token")}</Flex></Text>
  <Choice<OAuth2Auth["location"]> label={t("鉴权位置")} value={config.location} options={[{value:"header",label:"Header"},...(query?[{value:"query" as const,label:t("查询参数")}]:[])]} onChange={location=>update({location})}/>
  <Flex gap="3" wrap="wrap"><Field label={t("鉴权字段名")}><TextField.Root value={config.name} onChange={e=>update({name:e.target.value})}/></Field><Field label={t("Token 前缀")}><TextField.Root value={config.prefix} onChange={e=>update({prefix:e.target.value})}/></Field></Flex>
  <details><summary>{t("OAuth2 高级参数")}</summary>
   {url("revocation_url",t("Token 撤销 URL"))}{url("introspection_url",t("Token 检查 URL"))}
   <Field label={t("授权附加参数")}><PairEditor rows={config.authorization_params} onChange={authorization_params=>update({authorization_params})} secrets/></Field>
   <Field label={t("Token 附加参数")}><PairEditor rows={config.token_params} onChange={token_params=>update({token_params})} secrets/></Field>
   <Field label={t("Token 请求头")}><PairEditor rows={config.token_headers} onChange={token_headers=>update({token_headers})} secrets/></Field>
  </details>
  <TokenManager config={config} select={token_id=>update({token_id})}/>
  <Text size="1" color="gray">{t("Token 凭据独立保存在当前账户与工作区，不随工作区默认导出或同步。")}</Text>
 </Flex>;
}
