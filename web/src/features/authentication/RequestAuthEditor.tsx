import AsapAuthEditor,{asapConfig} from "./asap/AsapAuthEditor";
import EdgeGridAuthEditor,{edgeGridConfig} from "./edgegrid/EdgeGridAuthEditor";
import NtlmAuthEditor,{ntlmConfig} from "./ntlm/NtlmAuthEditor";
import OAuth1AuthEditor,{oauth1Config} from "./oauth1/OAuth1AuthEditor";
import HawkAuthEditor,{hawkConfig} from "./hawk/HawkAuthEditor";
import AwsAuthEditor,{awsConfig} from "./aws/AwsAuthEditor";
import OAuth2ConfigEditor from "./oauth2/OAuth2ConfigEditor";
import {oauth2Config} from "./oauth2/types";
import {TextField,TextArea,Checkbox,Flex,Text} from "@radix-ui/themes";
import {Choice,Field,Editor} from "../../shared/ui";
import {t,useLanguage} from "../../shared/i18n";
import type {Auth,JwtAuth,ApiKeyAuth} from "../../shared/types";
export const jwtConfig=():JwtAuth=>({algorithm:"HS256",key:"",key_base64:false,claims_source:"{}",kid:"",name:"Authorization",prefix:"Bearer",location:"header",add_time_claims:true,ttl_seconds:3600});
export const apiKeyConfig=():ApiKeyAuth=>({name:"X-API-Key",value:"",location:"header"});
export default function RequestAuthEditor({auth,change,dark,api=true,digest=true,query=true,inherit=true,credentials=true,collectionId,signing=true}:{auth:Auth;change:(value:Auth)=>void;dark:boolean;api?:boolean;digest?:boolean;query?:boolean;inherit?:boolean;credentials?:boolean;signing?:boolean;collectionId?:string|null}){
 useLanguage();const key=auth.api_key??apiKeyConfig(),jwt=auth.jwt??jwtConfig();
 function setKey(patch:Partial<ApiKeyAuth>){change({...auth,api_key:{...key,...patch}});}
 function setJwt(patch:Partial<JwtAuth>){change({...auth,jwt:{...jwt,...patch}});}
 return <div className="form-panel">
  <Choice label={t("鉴权类型")} value={auth.kind} options={[...(inherit?[{value:"inherit",label:t("继承父级鉴权")}]:[]),{value:"none",label:"No Auth"},...(credentials?[{value:"bearer",label:"Bearer Token"},{value:"basic",label:"Basic Auth"}]:[]),...(api?[{value:"apikey",label:"API Key"},{value:"jwt",label:"JWT"},{value:"oauth2",label:"OAuth 2.0"}]:[]),...(digest?[{value:"digest",label:"Digest Auth"},{value:"ntlm",label:"NTLM"}]:[]),...(signing?[{value:"asap",label:"Atlassian ASAP"},{value:"edgegrid",label:"Akamai EdgeGrid"},{value:"oauth1",label:"OAuth 1.0"},{value:"aws",label:"AWS Signature V4"},{value:"hawk",label:"Hawk"}]:[])]} onChange={kind=>change({...auth,kind:kind as Auth["kind"],...(kind==="apikey"?{api_key:key}:kind==="jwt"?{jwt}:kind==="oauth2"?{oauth2:auth.oauth2??oauth2Config()}:kind==="aws"?{aws:auth.aws??awsConfig()}:kind==="hawk"?{hawk:auth.hawk??hawkConfig()}:kind==="oauth1"?{oauth1:auth.oauth1??oauth1Config()}:kind==="ntlm"?{ntlm:auth.ntlm??ntlmConfig()}:kind==="edgegrid"?{edgegrid:auth.edgegrid??edgeGridConfig()}:kind==="asap"?{asap:auth.asap??asapConfig()}:{})})}/>
  {auth.kind==="asap"&&<AsapAuthEditor config={auth.asap??asapConfig()} change={asap=>change({...auth,asap})} dark={dark}/>}
  {auth.kind==="edgegrid"&&<EdgeGridAuthEditor config={auth.edgegrid??edgeGridConfig()} change={edgegrid=>change({...auth,edgegrid})}/>}
  {auth.kind==="oauth1"&&<OAuth1AuthEditor config={auth.oauth1??oauth1Config()} change={oauth1=>change({...auth,oauth1})} collectionId={collectionId}/>}
  {auth.kind==="hawk"&&<HawkAuthEditor config={auth.hawk??hawkConfig()} change={hawk=>change({...auth,hawk})}/>}
  {auth.kind==="aws"&&<AwsAuthEditor config={auth.aws??awsConfig()} change={aws=>change({...auth,aws})}/> }
  {auth.kind==="inherit"&&<Text size="1" color="gray">{t("执行时使用最近目录、集合或工作区的鉴权；No Auth 停止继承。")}</Text>}
  {auth.kind==="bearer"&&<Field label="Token" hint={t("可使用 {{api_token}} 引用环境变量。") }><TextField.Root type="password" autoComplete="off" value={auth.token} onChange={e=>change({...auth,token:e.target.value})}/></Field>}
  {["basic","digest","ntlm"].includes(auth.kind)&&<><Field label={t("用户名")}><TextField.Root autoComplete="off" value={auth.username} onChange={e=>change({...auth,username:e.target.value})}/></Field><Field label={t("密码")}><TextField.Root type="password" autoComplete="off" value={auth.password} onChange={e=>change({...auth,password:e.target.value})}/></Field></>}
  {auth.kind==="apikey"&&<><Choice<ApiKeyAuth["location"]> label={t("鉴权位置")} value={key.location} options={[{value:"header",label:"Header"},...(query?[{value:"query" as const,label:t("查询参数")}]:[])]} onChange={location=>setKey({location})}/><Field label={t("鉴权字段名")}><TextField.Root value={key.name} maxLength={512} onChange={e=>setKey({name:e.target.value})}/></Field><Field label={t("API Key 值")}><TextField.Root type="password" autoComplete="off" value={key.value} onChange={e=>setKey({value:e.target.value})}/></Field></>}
  {auth.kind==="jwt"&&<>
   <Flex gap="3" wrap="wrap"><Choice label={t("JWT 算法")} value={jwt.algorithm} options={["HS256","HS384","HS512","RS256","RS384","RS512","PS256","PS384","PS512","ES256","ES384","EdDSA"].map(value=>({value,label:value}))} onChange={algorithm=>setJwt({algorithm})}/><Choice<JwtAuth["location"]> label={t("鉴权位置")} value={jwt.location} options={[{value:"header",label:"Header"},...(query?[{value:"query" as const,label:t("查询参数")}]:[])]} onChange={location=>setJwt({location})}/></Flex>
   <Field label={t("JWT 签名密钥")} hint={t("HMAC 使用密钥文本；RSA、EC、EdDSA 使用私钥 PEM。不读取文件路径。") }><TextArea className="mono" autoComplete="off" value={jwt.key} maxLength={65536} onChange={e=>setJwt({key:e.target.value})}/></Field>
   {jwt.algorithm.startsWith("HS")&&<Text as="label" size="2"><Flex gap="2"><Checkbox checked={jwt.key_base64} onCheckedChange={v=>setJwt({key_base64:v===true})}/>{t("HMAC 密钥使用 Base64 编码")}</Flex></Text>}
   <Field label={t("JWT Claims JSON")}><Editor label={t("JWT Claims JSON")} value={jwt.claims_source} onChange={claims_source=>setJwt({claims_source})} dark={dark} jsonMode height="180px"/></Field>
   <Flex gap="3" wrap="wrap"><Field label={t("鉴权字段名")}><TextField.Root value={jwt.name} onChange={e=>setJwt({name:e.target.value})}/></Field><Field label={t("Token 前缀")}><TextField.Root value={jwt.prefix} onChange={e=>setJwt({prefix:e.target.value})}/></Field><Field label="kid"><TextField.Root value={jwt.kid} onChange={e=>setJwt({kid:e.target.value})}/></Field></Flex>
   <Text as="label" size="2"><Flex gap="2"><Checkbox checked={jwt.add_time_claims} onCheckedChange={v=>setJwt({add_time_claims:v===true})}/>{t("缺失时自动添加 iat 和 exp")}</Flex></Text>
   {jwt.add_time_claims&&<Field label={t("JWT 有效秒数")}><TextField.Root type="number" min={1} max={86400} value={jwt.ttl_seconds} onChange={e=>setJwt({ttl_seconds:Number(e.target.value)})}/></Field>}
  </>}
  {auth.kind==="oauth2"&&<OAuth2ConfigEditor config={auth.oauth2??oauth2Config()} change={oauth2=>change({...auth,oauth2})} query={query} collectionId={collectionId}/>}
  {auth.kind==="ntlm"&&<NtlmAuthEditor config={auth.ntlm??ntlmConfig()} change={ntlm=>change({...auth,ntlm})}/>}
  {auth.kind==="digest"&&<Text size="1" color="gray">{t("收到 401 Digest challenge 后自动签名并重试；跨域重定向不携带鉴权。")}</Text>}
  {["apikey","jwt"].includes(auth.kind)&&<Text size="1" color="gray">{t("鉴权在执行时解析环境变量。默认导出移除密钥；JWT 每次执行生成。")}</Text>}
 </div>;
}
