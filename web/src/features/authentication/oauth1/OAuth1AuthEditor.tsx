import {Checkbox,Flex,Text,TextArea,TextField} from "@radix-ui/themes";
import TokenManager from "./TokenManager";
import {Choice,Field,PairEditor} from "../../../shared/ui";
import {t,useLanguage} from "../../../shared/i18n";
import type {OAuth1Auth,OAuth1Grant} from "../../../shared/types";
export const oauth1GrantConfig=():OAuth1Grant=>({request_token_url:"",authorization_url:"",access_token_url:"",callback_url:"oob",request_params:[],access_params:[]});
export const oauth1Config=():OAuth1Auth=>({consumer_key:"",consumer_secret:"",token:"",token_secret:"",private_key:"",algorithm:"HMAC-SHA1",location:"header",realm:"",nonce:"",timestamp:"",callback:"",verifier:"",include_version:true,include_body_hash:false,include_empty_params:true});
export default function OAuth1AuthEditor({config,change,collectionId}:{config:OAuth1Auth;change:(value:OAuth1Auth)=>void;collectionId?:string|null}) {
 useLanguage();const update=(patch:Partial<OAuth1Auth>)=>change({...config,...patch}),rsa=config.algorithm.startsWith("RSA-");
 const grant=config.grant??oauth1GrantConfig();const updateGrant=(patch:Partial<OAuth1Grant>)=>update({grant:{...grant,...patch}});
 return <Flex direction="column" gap="3">
 <Choice label={t("OAuth1 签名算法")} value={config.algorithm} options={["HMAC-SHA1","HMAC-SHA256","HMAC-SHA512","RSA-SHA1","RSA-SHA256","RSA-SHA512","PLAINTEXT"].map(value=>({value,label:value}))} onChange={algorithm=>update({algorithm})}/>
 <Field label="Consumer Key"><TextField.Root autoComplete="off" value={config.consumer_key} maxLength={4096} onChange={e=>update({consumer_key:e.target.value})}/></Field>
 {rsa?<Field label={t("OAuth1 RSA 私钥 PEM")} hint={t("支持 PKCS8 和 PKCS1，密钥长度为 2048–4096 位。") }><TextArea className="mono" autoComplete="off" value={config.private_key} maxLength={16384} onChange={e=>update({private_key:e.target.value})}/></Field>:<Field label="Consumer Secret"><TextField.Root type="password" autoComplete="off" value={config.consumer_secret} maxLength={65536} onChange={e=>update({consumer_secret:e.target.value})}/></Field>}
 <Field label={t("OAuth1 Token（可选）")}><TextField.Root type="password" autoComplete="off" value={config.token} maxLength={4096} onChange={e=>update({token:e.target.value})}/></Field>
 {!rsa&&<Field label="Token Secret"><TextField.Root type="password" autoComplete="off" value={config.token_secret} maxLength={65536} onChange={e=>update({token_secret:e.target.value})}/></Field>}
 <Choice<OAuth1Auth["location"]> label={t("OAuth1 签名位置")} value={config.location} options={[{value:"header",label:"Header"},{value:"query",label:t("查询参数")},{value:"body",label:t("表单请求体")},{value:"automatic",label:t("Postman 自动位置") }]} onChange={location=>update({location})}/>
 {config.location==="body"&&<Text size="1" color="gray">{t("表单位置要求 application/x-www-form-urlencoded；保留原始表单字段。")}</Text>}
 {config.location==="automatic"&&<Text size="1" color="gray">{t("POST/PUT 表单使用请求体，其他请求使用查询参数。")}</Text>}
 {config.location==="header"&&<Field label="Realm"><TextField.Root value={config.realm} maxLength={4096} onChange={e=>update({realm:e.target.value})}/></Field>}
 <Text as="label" size="2"><Flex gap="2"><Checkbox checked={config.include_version} onCheckedChange={v=>update({include_version:v===true})}/>{t("包含 oauth_version=1.0")}</Flex></Text>
 <Text as="label" size="2"><Flex gap="2"><Checkbox checked={config.include_body_hash} onCheckedChange={v=>update({include_body_hash:v===true})}/>{t("包含非表单请求体哈希")}</Flex></Text>
 <Text as="label" size="2"><Flex gap="2"><Checkbox checked={config.include_empty_params} onCheckedChange={v=>update({include_empty_params:v===true})}/>{t("空值参数参与签名")}</Flex></Text>
 <details><summary>{t("高级 OAuth1 参数")}</summary><Flex direction="column" gap="3" mt="3">
 <><Field label="Nonce" hint={t("留空时每次请求生成新的随机值。") }><TextField.Root value={config.nonce} maxLength={4096} onChange={e=>update({nonce:e.target.value})}/></Field><Field label={t("OAuth1 时间戳")} hint={t("留空时使用当前 Unix 秒数，手动值必须为正整数。") }><TextField.Root value={config.timestamp} maxLength={128} onChange={e=>update({timestamp:e.target.value})}/></Field></>
 <Field label="Callback"><TextField.Root value={config.callback} maxLength={4096} onChange={e=>update({callback:e.target.value})}/></Field><Field label="Verifier"><TextField.Root type="password" autoComplete="off" value={config.verifier} maxLength={4096} onChange={e=>update({verifier:e.target.value})}/></Field>
 </Flex></details>
 {config.algorithm==="PLAINTEXT"&&<Text size="1" color="gray">{t("PLAINTEXT 签名包含共享密钥，请使用 HTTPS 保护传输。")}</Text>}
 <details><summary>{t("OAuth1 授权端点")}</summary><Flex direction="column" gap="3" mt="3">
 {([ ["request_token_url",t("请求 Token URL")],["authorization_url",t("授权 URL")],["access_token_url",t("访问 Token URL")],["callback_url",t("回调 URI")] ] as const).map(([key,label])=><Field key={key} label={label}><TextField.Root value={grant[key]} maxLength={8192} onChange={e=>updateGrant({[key]:e.target.value})}/></Field>)}
 <Field label={t("请求 Token 附加参数")}><PairEditor rows={grant.request_params} onChange={request_params=>updateGrant({request_params})} secrets/></Field>
 <Field label={t("访问 Token 附加参数")}><PairEditor rows={grant.access_params} onChange={access_params=>updateGrant({access_params})} secrets/></Field>
 </Flex></details>
 <TokenManager config={config} select={token_id=>update({token_id})} callback={callback_url=>updateGrant({callback_url})} collectionId={collectionId}/>
 <Text size="1" color="gray">{t("签名使用实际 URL 和请求体，支持环境变量及父级继承。选择私有 Token 后，执行时使用令牌库中的凭据。")}</Text>
 </Flex>;
}
