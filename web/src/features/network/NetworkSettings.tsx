import {useEffect,useRef,useState} from "react";
import {Button,Checkbox,Flex,Text,TextArea,TextField} from "@radix-ui/themes";
import {base64} from "@scure/base";
import {Choice,Field} from "../../shared/ui";
import {t,useLanguage} from "../../shared/i18n";
import type {RequestNetwork} from "../../shared/types";
import {pickBinaryFile} from "../../shared/pickBinaryFile";
export const defaultNetwork=():RequestNetwork=>({http_mode:"http1",proxy:{enabled:false,url:"",username:"",password:"",bypass:""},built_in_roots:true,ca_pem:"",identity:{enabled:false,format:"pem",certificate_pem:"",key_pem:"",pkcs12_base64:"",password:"",alias:""},dns:[],connect_timeout_ms:15000});
export default function NetworkSettings({value,change,disabled=false,ntlm=false,websocket=false,grpc=false,rawTcp=false,mqtt=false}:{value?:RequestNetwork;change:(value:RequestNetwork|undefined)=>void;disabled?:boolean;ntlm?:boolean;websocket?:boolean;grpc?:boolean;rawTcp?:boolean;mqtt?:boolean}){
 useLanguage();const c=value??{...defaultNetwork(),http_mode:grpc?"auto" as const:"http1" as const};const [picking,setPicking]=useState(false);const [error,setError]=useState(false);
 const current=useRef({value,disabled});current.current={value,disabled};const mounted=useRef(true);const generation=useRef(0);
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;generation.current++;};},[]);
 const update=(patch:Partial<RequestNetwork>)=>change({...c,...patch,...((rawTcp||mqtt)?{http_mode:"http1" as const}:{})});
 async function pick(field:"ca_pem"|"certificate_pem"|"key_pem"|"pkcs12_base64"){
   const snapshot=JSON.stringify(value);const ticket=++generation.current;setPicking(true);setError(false);
   try {const file=await pickBinaryFile({extensions:field==="pkcs12_base64"?["p12","pfx"]:["pem","crt","cer","key"],accept:field==="pkcs12_base64"?".p12,.pfx":".pem,.crt,.cer,.key"});
    if(!file||!mounted.current||ticket!==generation.current||current.current.disabled||JSON.stringify(current.current.value)!==snapshot)return;
    const bytes=base64.decode(file.base64);const limit=field==="pkcs12_base64"?192*1024:field==="key_pem"?65536:128*1024;
    if(bytes.length>limit)throw new Error("limit");
    const text=field==="pkcs12_base64"?file.base64:new TextDecoder("utf-8",{fatal:true}).decode(bytes);
    if(field==="ca_pem")update({ca_pem:text});else update({identity:{...c.identity,[field]:text}});
   }catch{if(mounted.current&&ticket===generation.current&&JSON.stringify(current.current.value)===snapshot)setError(true);}
   finally{if(mounted.current&&ticket===generation.current)setPicking(false);}
 }
 const toggle=(label:string,checked:boolean,onChange:(value:boolean)=>void)=><label className="checkbox-label"><Checkbox disabled={disabled||picking} checked={checked} onCheckedChange={v=>onChange(v===true)}/>{label}</label>;
 const pem=(label:string,field:"ca_pem"|"certificate_pem"|"key_pem",text:string,onChange:(value:string)=>void)=><Flex direction="column" gap="2"><Field label={label}><TextArea disabled={disabled||picking} rows={3} autoComplete="off" value={text} maxLength={field==="key_pem"?65536:128*1024} onChange={e=>onChange(e.target.value)}/></Field><Button type="button" variant="soft" disabled={disabled||picking} onClick={()=>void pick(field)}>{t("选择证书文件")}</Button></Flex>;
 return <details><summary>{t("请求网络设置")}</summary><Flex direction="column" gap="3" mt="3">
 <Text size="1" color="gray">{t("适用于 HTTP、SOAP、SSE、WebSocket、GraphQL、gRPC、TCP、MQTT、Socket.IO、A2A 和 MCP HTTP 请求。支持环境变量；停用后保留草稿。")}</Text>
 {!rawTcp&&!mqtt&&<Field label={t("HTTP 版本")}><Choice disabled={disabled||ntlm||websocket} label={t("HTTP 版本")} value={c.http_mode} onChange={http_mode=>update({http_mode})} options={grpc?[{value:"auto",label:"HTTP/2"},{value:"http2_prior_knowledge",label:"HTTP/2 prior knowledge"}]:[{value:"http1",label:"HTTP/1.1"},{value:"auto",label:t("自动协商 HTTP/2")},{value:"http2_prior_knowledge",label:"HTTP/2 prior knowledge"}]}/></Field>}
 {mqtt&&<Text size="1" color="gray">{t("MQTT 的 TCP/TLS 和 WebSocket 传输共用网络设置，WebSocket 升级使用 HTTP/1.1。")}</Text>}
 {rawTcp&&<Text size="1" color="gray">{t("TCP 使用原始套接字；tcps:// 支持 CA 和客户端证书。")}</Text>}
 {grpc&&<Text size="1" color="gray">{t("gRPC 使用 HTTP/2；TLS、DNS 和代理配置同样适用。")}</Text>}
 {websocket&&<Text size="1" color="gray">{t("WebSocket 升级需要 HTTP/1.1。")}</Text>}
 {ntlm&&<Text size="1" color="gray">{t("NTLM 需要 HTTP/1.1。")}</Text>}
 <Field label={t("连接超时 (ms)")}><TextField.Root disabled={disabled} type="number" min={100} max={120000} value={c.connect_timeout_ms} onChange={e=>update({connect_timeout_ms:Number(e.target.value)})}/></Field>
 {toggle(t("使用请求代理"),c.proxy.enabled,enabled=>update({proxy:{...c.proxy,enabled}}))}
 {c.proxy.enabled&&<Flex direction="column" gap="3">
 <Text size="1" color="gray">{t("HTTP/HTTPS/SOCKS 代理。托管服务需管理员允许私网访问；代理负责远端解析。")}</Text>
 <Field label={t("代理地址")}><TextField.Root disabled={disabled} maxLength={8192} placeholder="http://127.0.0.1:7890" value={c.proxy.url} onChange={e=>update({proxy:{...c.proxy,url:e.target.value}})}/></Field>
 <Field label={t("代理用户名")}><TextField.Root disabled={disabled} autoComplete="off" value={c.proxy.username} maxLength={4096} onChange={e=>update({proxy:{...c.proxy,username:e.target.value}})}/></Field>
 <Field label={t("代理密码")}><TextField.Root disabled={disabled} type="password" autoComplete="off" value={c.proxy.password} maxLength={65536} onChange={e=>update({proxy:{...c.proxy,password:e.target.value}})}/></Field>
 <Field label={t("代理绕过规则")} hint={t("英文逗号分隔域名、IP 或 CIDR；* 绕过所有地址。") }><TextField.Root disabled={disabled} value={c.proxy.bypass} maxLength={8192} onChange={e=>update({proxy:{...c.proxy,bypass:e.target.value}})}/></Field>
 </Flex>}
 <details><summary>{t("TLS 与客户端证书")}</summary><Flex direction="column" gap="3" mt="3">
 {toggle(t("使用内置 CA 根证书"),c.built_in_roots,built_in_roots=>update({built_in_roots}))}
 {pem(t("附加 CA 证书 (PEM)"),"ca_pem",c.ca_pem,ca_pem=>update({ca_pem}))}
 {toggle(t("使用客户端证书 (mTLS)"),c.identity.enabled,enabled=>update({identity:{...c.identity,enabled}}))}
 {c.identity.enabled&&<Flex direction="column" gap="3">
 <Field label={t("客户端证书格式")}><Choice label={t("客户端证书格式")} disabled={disabled||picking} value={c.identity.format} onChange={format=>update({identity:{...c.identity,format}})} options={[{value:"pem",label:"PEM"},{value:"pkcs12",label:"PKCS#12 / PFX"}]}/></Field>
 {c.identity.format==="pem"?<>
 {pem(t("客户端证书链 (PEM)"),"certificate_pem",c.identity.certificate_pem,certificate_pem=>update({identity:{...c.identity,certificate_pem}}))}
 {pem(t("客户端私钥 (PEM)"),"key_pem",c.identity.key_pem,key_pem=>update({identity:{...c.identity,key_pem}}))}
 </>:<>
 <Field label={t("PFX 内容 (Base64)")}><TextArea disabled={disabled||picking} rows={3} autoComplete="off" value={c.identity.pkcs12_base64} maxLength={256*1024} onChange={e=>update({identity:{...c.identity,pkcs12_base64:e.target.value}})}/></Field>
 <Button type="button" variant="soft" disabled={disabled||picking} onClick={()=>void pick("pkcs12_base64")}>{t("选择 PFX 文件")}</Button>
 <Field label={t("PFX 密码")}><TextField.Root disabled={disabled} type="password" autoComplete="off" maxLength={4096} value={c.identity.password} onChange={e=>update({identity:{...c.identity,password:e.target.value}})}/></Field>
 <Field label={t("PFX 身份别名（可选）")} hint={t("只有一个身份时可留空；多个身份需填写别名。") }><TextField.Root disabled={disabled} value={c.identity.alias} maxLength={512} onChange={e=>update({identity:{...c.identity,alias:e.target.value}})}/></Field>
 </>}
 <Text size="1" color="gray">{t("证书与密钥保存在请求源中；默认导出会移除，需显式包含敏感信息进行备份。")}</Text>
 </Flex>}
 </Flex></details>
 <details><summary>{t("DNS 覆盖")}</summary><Flex direction="column" gap="3" mt="3">
 <Text size="1" color="gray">{t("将主机映射到 IP，保留原始 Host 与 TLS 名称。受限服务端仍会拒绝私网地址。")}</Text>
 {c.dns.map((entry,index)=><Flex direction="column" gap="2" key={index}>
 <Field label={t("主机名")}><TextField.Root disabled={disabled} value={entry.hostname} maxLength={253} onChange={e=>update({dns:c.dns.map((v,i)=>i===index?{...v,hostname:e.target.value}:v)})}/></Field>
 <Field label={t("IP 地址（英文逗号分隔）")}><TextField.Root disabled={disabled} value={entry.addresses.join(",")} onChange={e=>update({dns:c.dns.map((v,i)=>i===index?{...v,addresses:e.target.value.split(",").map(v=>v.trim())}:v)})}/></Field>
 <Button type="button" variant="soft" color="red" disabled={disabled} onClick={()=>update({dns:c.dns.filter((_,i)=>i!==index)})}>{t("移除 DNS 覆盖")}</Button>
 </Flex>)}
 <Button type="button" variant="soft" disabled={disabled||c.dns.length>=64} onClick={()=>update({dns:[...c.dns,{hostname:"",addresses:[]}]})}>{t("添加 DNS 覆盖")}</Button>
 </Flex></details>
 {picking&&<Text role="status" size="1">{t("正在读取证书文件…")}</Text>}
 {error&&<Text role="alert" color="red" size="1">{t("无法读取证书文件：检查编码及大小（PEM 128 KiB、私钥 64 KiB、PFX 192 KiB）。")}</Text>}
 <Button type="button" variant="ghost" disabled={disabled||picking||!value} onClick={()=>change(undefined)}>{t("重置网络设置")}</Button>
 </Flex></details>;
}
