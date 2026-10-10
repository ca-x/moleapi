import {Checkbox,Flex,Text} from "@radix-ui/themes";
import {t,useLanguage} from "../../shared/i18n";
import type {Collection} from "../../shared/types";
import {collectionDescendants} from "../collections/tree";
export default function CiRequestPicker({collections,root,value,onChange}:{collections:Collection[];root:string;value:string[];onChange:(value:string[])=>void}){
 useLanguage();const descendants=collectionDescendants(collections,root);
 return <Flex direction="column" gap="2"><Text size="2">{t("CI 接口筛选")}</Text><Text size="1" color="gray">{t("选择接口可限制执行范围；留空时运行整个集合及子目录。")}</Text>{collections.filter(collection=>descendants.has(collection.id)).map(collection=><Flex key={collection.id} direction="column" gap="1"><Text size="1" color="gray">{collection.name}</Text>{collection.requests.map(request=><label key={request.id} className="checkbox-label"><Checkbox checked={value.includes(request.id)} disabled={!value.includes(request.id)&&value.length>=1000} onCheckedChange={checked=>onChange(checked===true?[...value,request.id]:value.filter(id=>id!==request.id))}/>{request.method} {request.name}</label>)}</Flex>)}</Flex>;
}
