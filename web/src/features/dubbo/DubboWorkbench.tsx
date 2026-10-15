import {Button, Checkbox, Flex, Separator, Text, TextArea, TextField} from "@radix-ui/themes";
import {t, useLanguage} from "../../shared/i18n";
import {id} from "../../shared/model";
import type {DubboConfig, RequestSpec} from "../../shared/types";
import {Choice, Field, PairEditor} from "../../shared/ui";

export default function DubboWorkbench({request, update, disabled}:{request:RequestSpec;update:(patch:Partial<RequestSpec>)=>void;disabled:boolean}) {
  useLanguage();
  if (request.protocol?.kind !== "dubbo") return null;
  const config = request.protocol;
  const change = (patch:Partial<DubboConfig>) => update({protocol:{...config,...patch}});
  const source = config.source ?? "direct";
  const changeAuth = (username:string, password:string) =>
    change({registry_auth:username || password ? {username,password} : undefined});
  const move = (index:number, offset:number) => {
    const arguments0 = [...config.arguments];
    [arguments0[index], arguments0[index + offset]] = [arguments0[index + offset], arguments0[index]];
    change({arguments:arguments0});
  };
  return <div className="form-panel">
    <Text size="2" color="gray">{t("使用经典 Dubbo/Hessian2。服务接口和参数类型必须与提供方的方法声明一致。")}</Text>
    <Field label={t("Dubbo 来源")}>
      <Choice value={source} disabled={disabled} label={t("Dubbo 来源")}
        options={[{value:"direct",label:t("直连提供方")},{value:"zookeeper",label:"ZooKeeper"},{value:"nacos",label:"Nacos"}]}
        onChange={value=>update({
          protocol:{...config,source:value,source_version:value==="direct"?1:2,
            registry_auth:value===source?config.registry_auth:undefined,
            registry_namespace:value==="nacos"?config.registry_namespace??"":""},
          url:value==="direct"?"dubbo://localhost:20880":value==="zookeeper"?"zookeeper://localhost:2181":"nacos://localhost:8848",
        })}/>
    </Field>
    {source==="zookeeper" && <>
      <Text size="2" color="gray">{t("在上方 URL 栏填写 ZooKeeper 地址。每次调用都会重新查找提供方。")}</Text>
      <Flex gap="3" wrap="wrap">
        <Field label={t("ZooKeeper 用户名")}><TextField.Root aria-label={t("ZooKeeper 用户名")} value={config.registry_auth?.username ?? ""} maxLength={128} disabled={disabled} onChange={event=>changeAuth(event.target.value,config.registry_auth?.password ?? "")}/></Field>
        <Field label={t("ZooKeeper 密码")}><TextField.Root type="password" aria-label={t("ZooKeeper 密码")} value={config.registry_auth?.password ?? ""} maxLength={4096} disabled={disabled} onChange={event=>changeAuth(config.registry_auth?.username ?? "",event.target.value)}/></Field>
      </Flex>
      <Text size="2" color="gray">{t("凭据留空表示无需认证；填写时使用 ZooKeeper digest 认证。")}</Text>
    </>}
    {source==="nacos" && <>
      <Text size="2" color="gray">{t("在上方 URL 栏填写 Nacos 地址。命名空间 ID 留空表示 public。")}</Text>
      <Field label={t("Nacos 命名空间 ID")}><TextField.Root aria-label={t("Nacos 命名空间 ID")} value={config.registry_namespace ?? ""} maxLength={128} disabled={disabled} placeholder="team-qa" onChange={event=>change({registry_namespace:event.target.value})}/></Field>
      <Flex gap="3" wrap="wrap">
        <Field label={t("Nacos 用户名")}><TextField.Root aria-label={t("Nacos 用户名")} value={config.registry_auth?.username ?? ""} maxLength={128} disabled={disabled} onChange={event=>changeAuth(event.target.value,config.registry_auth?.password ?? "")}/></Field>
        <Field label={t("Nacos 密码")}><TextField.Root type="password" aria-label={t("Nacos 密码")} value={config.registry_auth?.password ?? ""} maxLength={4096} disabled={disabled} onChange={event=>changeAuth(config.registry_auth?.username ?? "",event.target.value)}/></Field>
      </Flex>
      <Text size="2" color="gray">{t("Nacos 凭据留空表示无需认证；HTTP 认证仅支持私网地址。")}</Text>
    </>}
    <Flex gap="3" wrap="wrap">
      <Field label={t("Dubbo 服务")}><TextField.Root aria-label={t("Dubbo 服务")} value={config.service} maxLength={256} disabled={disabled} placeholder="com.example.DemoService" onChange={event=>change({service:event.target.value})}/></Field>
      <Field label={t("Dubbo 方法")}><TextField.Root aria-label={t("Dubbo 方法")} value={config.method} maxLength={128} disabled={disabled} placeholder="sayHello" onChange={event=>change({method:event.target.value})}/></Field>
      <Field label={t("Dubbo 版本")}><TextField.Root aria-label={t("Dubbo 版本")} value={config.version} maxLength={128} disabled={disabled} placeholder="1.0.0" onChange={event=>change({version:event.target.value})}/></Field>
      <Field label={t("Dubbo 分组")}><TextField.Root aria-label={t("Dubbo 分组")} value={config.group} maxLength={128} disabled={disabled} onChange={event=>change({group:event.target.value})}/></Field>
    </Flex>
    <Separator size="4"/>
    <Flex justify="between" align="center" wrap="wrap" gap="2"><Text weight="medium">{t("Dubbo 参数")}</Text><Button size="1" variant="soft" color="gray" disabled={disabled||config.arguments.length>=32} onClick={()=>change({arguments:[...config.arguments,{id:id(),type_name:"java.lang.String",value_source:'""',secret:false}]})}>{t("添加参数")}</Button></Flex>
    {config.arguments.map((argument,index)=><Flex key={argument.id} direction="column" gap="2" style={{minWidth:0}}>
      <Flex gap="2" wrap="wrap" align="end"><Field label={t("Java 参数类型")}><TextField.Root aria-label={t("Java 参数类型")} value={argument.type_name} maxLength={256} disabled={disabled} placeholder="java.lang.String" onChange={event=>change({arguments:config.arguments.map(row=>row.id===argument.id?{...row,type_name:event.target.value}:row)})}/></Field>
        <Button size="1" variant="soft" color="gray" disabled={disabled||index===0} aria-label={t("上移参数")} onClick={()=>move(index,-1)}>{t("上移")}</Button>
        <Button size="1" variant="soft" color="gray" disabled={disabled||index===config.arguments.length-1} aria-label={t("下移参数")} onClick={()=>move(index,1)}>{t("下移")}</Button>
        <Button size="1" variant="soft" color="red" disabled={disabled} aria-label={t("删除参数")} onClick={()=>change({arguments:config.arguments.filter(row=>row.id!==argument.id)})}>{t("删除")}</Button>
      </Flex>
      <Field label={t("参数 JSON")}><TextArea aria-label={t("参数 JSON")} className="mono" rows={3} maxLength={65536} disabled={disabled} value={argument.value_source} onChange={event=>change({arguments:config.arguments.map(row=>row.id===argument.id?{...row,value_source:event.target.value}:row)})}/></Field>
      <label className="checkbox-label"><Checkbox disabled={disabled} checked={argument.secret} onCheckedChange={value=>change({arguments:config.arguments.map(row=>row.id===argument.id?{...row,secret:value===true}:row)})}/>{t("私密参数")}</label>
      <Separator size="4"/>
    </Flex>)}
    <Text size="2" color="gray">{t("64 位整数请写为 JSON 字符串；复杂 DTO 可使用 $class/$ 类型包装。")}</Text>
    <Text weight="medium">{t("Dubbo 附件")}</Text>
    <PairEditor rows={config.attachments} onChange={attachments=>change({attachments})} disabled={disabled} secrets keyLabel={t("附件名")} valueLabel={t("附件值")}/>
  </div>;
}
