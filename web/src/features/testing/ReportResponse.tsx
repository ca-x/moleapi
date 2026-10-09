import {useQuery} from "@tanstack/react-query";
import {Button,Flex,Text} from "@radix-ui/themes";
import {api} from "../../shared/api";
import {t,useLanguage} from "../../shared/i18n";
import type {HistoryEntry} from "../../shared/types";
import {ResponsePane} from "../requests/ResponsePane";
export default function ReportResponse({accountId,workspaceId,reportId,position,dark,onClose}:{accountId:string;workspaceId:string;reportId:string;position:number;dark:boolean;onClose:()=>void}){
 useLanguage();
 const response=useQuery({queryKey:["report-response",accountId,workspaceId,reportId,position],queryFn:()=>api<HistoryEntry>(`/api/workspaces/${workspaceId}/reports/${reportId}/steps/${position}/response`),retry:false});
 const missing=(response.error as {status?:number}|null)?.status===404;
 return <section><Flex justify="between" align="center" gap="3" wrap="wrap" my="3"><Text weight="medium">{t("步骤 {{position}} 的历史响应",{position:position+1})}</Text><Button variant="soft" onClick={onClose}>{t("关闭历史响应")}</Button></Flex>
 <Text size="1" color="gray">{t("读取运行时保存的脱敏历史，不会重新发送请求。清空历史后，此响应链接不可用，报告结果仍保留。")}</Text>
 <ResponsePane response={response.data?.response??null} error={missing?t("历史响应已删除或不可用，报告结果仍然保留。"):response.error?t("无法读取历史响应。"):""} dark={dark} busy={response.isPending}/>
 </section>;
}
