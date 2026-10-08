import {useEffect, useRef, useState} from "react";
import {Button, Callout, Checkbox, Dialog, Flex, Table, Text, TextField} from "@radix-ui/themes";
import {api} from "../../shared/api";
import {Field} from "../../shared/ui";
import {t, useLanguage} from "../../shared/i18n";
import {safeMessage} from "../../shared/model";
interface CookieEntry {domain:string;path:string;name:string;value:string;host_only:boolean;secure:boolean;http_only:boolean;same_site:string|null;expires:string}
interface Snapshot {enabled:boolean;cookies:CookieEntry[]}
export default function CookieManager({workspace,environment,url}:{workspace:string;environment?:string|null;url:string}) {
 useLanguage();
 const [open,setOpen]=useState(false);
 const [snapshot,setSnapshot]=useState<Snapshot|null>(null);
 const [busy,setBusy]=useState(false);
 const [error,setError]=useState("");
 const [origin,setOrigin]=useState(url);
 const [cookie,setCookie]=useState("");
 const [reveal,setReveal]=useState(false);
 const generation=useRef(0);
 const path=`/api/workspaces/${encodeURIComponent(workspace)}/cookies${environment?`?environment_id=${encodeURIComponent(environment)}`:""}`;
 useEffect(()=>()=>{generation.current++;},[]);
 const action=async(method="GET",body?:unknown,show=false)=>{
   const current=generation.current;setBusy(true);setError("");
   try {const result=await api<Snapshot>(`${path}${show?`${environment?"&":"?"}reveal=true`:""}`,method,body);if(generation.current===current){setSnapshot(result);setReveal(show);if(method==="POST")setCookie("");}}
   catch(e){if(generation.current===current)setError(safeMessage(e));}
   finally{if(generation.current===current)setBusy(false);}
 };
 return <Dialog.Root open={open} onOpenChange={value=>{generation.current++;setOpen(value);setSnapshot(null);setReveal(false);setCookie("");setError("");setBusy(false);if(value){setOrigin(url);void action();}}}>
 <Dialog.Trigger><Button variant="soft" color="gray">{t("管理 Cookie")}</Button></Dialog.Trigger>
 <Dialog.Content maxWidth="880px" style={{width:"calc(100vw - 32px)"}}><Dialog.Title>{t("管理 Cookie")}</Dialog.Title><Dialog.Description>{t("Cookie 按账号、工作区和当前环境隔离，仅保存在进程内存中；退出账号或重启后清除，不参与同步和导出。")}</Dialog.Description>
 <Flex direction="column" gap="3" mt="3" style={{minWidth:0}}>
 <label className="checkbox-label"><Checkbox checked={snapshot?.enabled??false} disabled={busy||!snapshot} onCheckedChange={v=>void action("PATCH",{enabled:v===true})}/>{t("自动接收和发送 Cookie")}</label>
 <Text size="2" color="gray">{t("用于 HTTP、SOAP 和 GraphQL 查询；手动 Cookie Header 优先。其他协议不使用此 Cookie 库。")}</Text>
 <Flex gap="2" wrap="wrap"><Button variant="soft" disabled={busy} onClick={()=>void action("GET",undefined,reveal)}>{t("刷新")}</Button><Button variant="soft" disabled={busy||!snapshot} onClick={()=>void action("GET",undefined,!reveal)}>{reveal?t("隐藏 Cookie 值"):t("显示 Cookie 值")}</Button><Button variant="soft" color="red" disabled={busy||!snapshot} onClick={()=>void action("PATCH",{enabled:snapshot?.enabled,clear:true})}>{t("清空 Cookie")}</Button></Flex>
 {snapshot&&<div style={{overflowX:"auto",maxHeight:300,minWidth:0,maxWidth:"100%"}}><Table.Root size="1" style={{minWidth:640}}><Table.Header><Table.Row><Table.ColumnHeaderCell>{t("名称")}</Table.ColumnHeaderCell><Table.ColumnHeaderCell>{t("值")}</Table.ColumnHeaderCell><Table.ColumnHeaderCell>{t("域名 / 路径")}</Table.ColumnHeaderCell><Table.ColumnHeaderCell>{t("属性")}</Table.ColumnHeaderCell><Table.ColumnHeaderCell>{t("操作")}</Table.ColumnHeaderCell></Table.Row></Table.Header><Table.Body>
 {snapshot.cookies.map(row=><Table.Row key={JSON.stringify([row.domain,row.path,row.name])}><Table.Cell>{row.name}</Table.Cell><Table.Cell><TextField.Root aria-label={t("Cookie 值：{{value0}}",{value0:row.name})} readOnly type={reveal?"text":"password"} value={row.value}/></Table.Cell><Table.Cell>{row.domain}<br/>{row.path}</Table.Cell><Table.Cell>{[row.host_only?"HostOnly":"Domain",row.secure?"Secure":"",row.http_only?"HttpOnly":"",row.same_site?`SameSite=${row.same_site}`:"",row.expires].filter(Boolean).join(" · ")}</Table.Cell><Table.Cell><Button size="1" variant="soft" color="red" disabled={busy} aria-label={t("删除 Cookie：{{value0}}",{value0:row.name})} onClick={()=>void action("DELETE",{domain:row.domain,path:row.path,name:row.name})}>{t("删除")}</Button></Table.Cell></Table.Row>)}
 </Table.Body></Table.Root>{snapshot.cookies.length===0&&<Text as="p" size="2" color="gray">{t("当前环境没有 Cookie。")}</Text>}</div>}
 <Field label={t("Cookie 来源 URL")}><TextField.Root value={origin} disabled={busy} onChange={e=>setOrigin(e.target.value)} placeholder="https://api.example.com/"/></Field>
 <Field label="Set-Cookie" hint={t("输入一个完整 Set-Cookie 值，例如 sid=value; Path=/; HttpOnly。再次添加相同域名、路径和名称可更新 Cookie。") }><TextField.Root type="password" autoComplete="off" maxLength={8192} disabled={busy} value={cookie} onChange={e=>setCookie(e.target.value)}/></Field>
 <Button disabled={busy||!cookie||!origin} onClick={()=>void action("POST",{url:origin,cookie})}>{t("添加 Cookie")}</Button>
 {busy&&<Text role="status" size="2">{t("正在加载…")}</Text>}{error&&<Callout.Root color="red" role="alert"><Callout.Text>{error}</Callout.Text></Callout.Root>}
 <Flex justify="end"><Dialog.Close><Button variant="soft" color="gray">{t("完成")}</Button></Dialog.Close></Flex>
 </Flex></Dialog.Content></Dialog.Root>;
}
