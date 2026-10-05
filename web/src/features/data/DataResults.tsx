import {useMemo,useState} from "react";
import {useReactTable,getCoreRowModel,getSortedRowModel,getFilteredRowModel,getPaginationRowModel,flexRender,type ColumnDef,type SortingState} from "@tanstack/react-table";
import Decimal from "decimal.js";
import {Button,Flex,Table,Text,TextField,Badge} from "@radix-ui/themes";
import {toast} from "sonner";
import {saveFile} from "../../shared/api";
import {exportResult} from "./export";
import {t,useLanguage} from "../../shared/i18n";
import type {Result,Cell} from "./types";
import {cellText} from "./model";
export function DataResults({result}:{result:Result|null}) {
  useLanguage();const [sorting,setSorting]=useState<SortingState>([]),[filter,setFilter]=useState("");
  const columns=useMemo<ColumnDef<Cell[]>[]>(()=>result?.columns.map((column,index)=>({id:String(index),header:column.name,accessorFn:row=>cellText(row[index]??{kind:"null"}),cell:context=>{const cell=context.row.original[index];return cell?.kind==="null"?<Text color="gray">NULL</Text>:<span title={column.data_type}>{String(context.getValue())}</span>;},sortingFn:(a,b)=>{const ac=a.original[index],bc=b.original[index];if(ac&&bc&&["integer","decimal"].includes(ac.kind)&&["integer","decimal"].includes(bc.kind)){try{return new Decimal(cellText(ac)).cmp(new Decimal(cellText(bc)))||0;}catch{/* Preserve non-finite/foreign values. */}}return cellText(ac??{kind:"null"}).localeCompare(cellText(bc??{kind:"null"}));}}))??[],[result?.columns]);
  const table=useReactTable({data:result?.rows??[],columns,state:{sorting,globalFilter:filter},onSortingChange:setSorting,onGlobalFilterChange:setFilter,getCoreRowModel:getCoreRowModel(),getSortedRowModel:getSortedRowModel(),getFilteredRowModel:getFilteredRowModel(),getPaginationRowModel:getPaginationRowModel(),initialState:{pagination:{pageSize:25}}});
  async function download(format:"json"|"csv") {
    if(!result?.done)return;
    try {await saveFile(exportResult(result,format));}
    catch {toast.error(t("无法保存查询结果"));}
  }
  if(!result)return <Text color="gray">{t("运行查询后查看数据。")}</Text>;
  return <div className="data-results">
    <Flex gap="3" wrap="wrap" align="center"><Text size="2">{t("已返回 {{count}} 行",{count:result.rows.length})} · {result.elapsed_ms} ms</Text><Text size="2">{t("影响 {{count}} 行",{count:result.rows_affected})}</Text>{!result.done&&<Badge color="amber">{t("查询中")}</Badge>}{result.truncated&&<Badge color="amber">{t("结果已截断")}</Badge>}<TextField.Root aria-label={t("搜索结果")} placeholder={t("搜索结果")} value={filter} onChange={e=>setFilter(e.target.value)}/></Flex>
    {result.limit_reason&&<Text color="gray" size="2">{result.limit_reason}</Text>}
    <Flex gap="2" wrap="wrap" align="center"><Button size="1" variant="soft" disabled={!result.done} onClick={()=>void download("json")}>{t("导出 JSON")}</Button><Button size="1" variant="soft" disabled={!result.done} onClick={()=>void download("csv")}>{t("导出 CSV")}</Button><Text size="1" color="gray">{t("导出全部已返回行。JSON 保留类型；CSV 空单元格表示 NULL，公式会转义。")}</Text></Flex>
    <div className="data-table-scroll" tabIndex={0} role="region" aria-label={t("查询结果表")}><Table.Root><Table.Header>{table.getHeaderGroups().map(group=><Table.Row key={group.id}>{group.headers.map(header=><Table.ColumnHeaderCell key={header.id} aria-sort={header.column.getIsSorted()==="asc"?"ascending":header.column.getIsSorted()==="desc"?"descending":"none"}><button className="data-sort" onClick={header.column.getToggleSortingHandler()}>{flexRender(header.column.columnDef.header,header.getContext())}{header.column.getIsSorted()==="asc"?" ↑":header.column.getIsSorted()==="desc"?" ↓":""}</button></Table.ColumnHeaderCell>)}</Table.Row>)}</Table.Header><Table.Body>{table.getRowModel().rows.map(row=><Table.Row key={row.id}>{row.getVisibleCells().map(cell=><Table.Cell key={cell.id}>{flexRender(cell.column.columnDef.cell,cell.getContext())}</Table.Cell>)}</Table.Row>)}</Table.Body></Table.Root></div>
    <Flex gap="3" align="center" wrap="wrap"><Button variant="soft" disabled={!table.getCanPreviousPage()} onClick={()=>table.previousPage()}>{t("上一页")}</Button><Text size="2">{t("第 {{page}} 页 / {{total}} 页",{page:table.getState().pagination.pageIndex+1,total:Math.max(table.getPageCount(),1)})}</Text><Button variant="soft" disabled={!table.getCanNextPage()} onClick={()=>table.nextPage()}>{t("下一页")}</Button></Flex>
  </div>;
}
