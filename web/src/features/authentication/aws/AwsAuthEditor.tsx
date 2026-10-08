import {Checkbox,Flex,Text,TextField} from "@radix-ui/themes";
import {Choice,Field} from "../../../shared/ui";
import {t,useLanguage} from "../../../shared/i18n";
import type {AwsAuth} from "../../../shared/types";
export const awsConfig=():AwsAuth=>({access_key:"",secret_key:"",session_token:"",region:"us-east-1",service:"execute-api",location:"header",expires_seconds:900,unsigned_payload:false});
export default function AwsAuthEditor({config,change}:{config:AwsAuth;change:(value:AwsAuth)=>void}) {
 useLanguage();const s3Presign=config.service==="s3"&&config.location==="query";const update=(patch:Partial<AwsAuth>)=>change({...config,...patch});
 return <Flex direction="column" gap="3">
 <Field label="Access Key ID"><TextField.Root autoComplete="off" value={config.access_key} maxLength={4096} onChange={e=>update({access_key:e.target.value})}/></Field>
 <Field label="Secret Access Key"><TextField.Root type="password" autoComplete="off" value={config.secret_key} maxLength={65536} onChange={e=>update({secret_key:e.target.value})}/></Field>
 <Field label={t("AWS 会话 Token（可选）")}><TextField.Root type="password" autoComplete="off" value={config.session_token} maxLength={65536} onChange={e=>update({session_token:e.target.value})}/></Field>
 <Flex gap="3" wrap="wrap"><Field label={t("AWS 区域")}><TextField.Root value={config.region} maxLength={128} onChange={e=>update({region:e.target.value})}/></Field><Field label={t("AWS 服务")}><TextField.Root value={config.service} maxLength={128} onChange={e=>update({service:e.target.value})}/></Field></Flex>
 <Choice<AwsAuth["location"]> label={t("AWS 签名位置")} value={config.location} options={[{value:"header",label:"Header"},{value:"query",label:t("预签名查询参数")}]} onChange={location=>update({location})}/>
 {config.location==="query"&&<Field label={t("AWS 预签名有效秒数")}><TextField.Root type="number" min={1} max={604800} value={config.expires_seconds} onChange={e=>update({expires_seconds:Number(e.target.value)})}/></Field>}
 <Text as="label" size="2"><Flex gap="2"><Checkbox checked={config.unsigned_payload||s3Presign} disabled={s3Presign} onCheckedChange={value=>update({unsigned_payload:value===true})}/>{t("使用 UNSIGNED-PAYLOAD")}</Flex></Text>
 {config.location==="query"&&<Text size="1" color="gray">{t("S3 预签名自动使用 UNSIGNED-PAYLOAD。")}</Text>}
 <Text size="1" color="gray">{t("根据实际发送的请求生成签名。凭据可引用环境变量；S3 自动使用专用路径和校验设置。")}</Text>
 </Flex>;
}
