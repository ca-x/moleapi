import { useState } from "react";
import { Button, Dialog, Flex, Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import { Editor } from "../../shared/ui";

function invalidSchemaInput(source:string):boolean {
  if(new TextEncoder().encode(source).length>256*1024)return true;
  try { const value=JSON.parse(source); return !value||typeof value!=="object"||Array.isArray(value); }
  catch { return true; }
}

export default function SchemaEditorDialog({schema,dark,onCommit}:{schema:string;dark:boolean;onCommit:(schema:string)=>void}) {
  useLanguage();
  const [open,setOpen]=useState(false);
  const [draft,setDraft]=useState(schema);
  return <Dialog.Root open={open} onOpenChange={next=>{if(next)setDraft(schema);setOpen(next);}}>
    <Dialog.Trigger><Button variant="soft" color="gray">{t("编辑 Mock JSON Schema")}</Button></Dialog.Trigger>
    <Dialog.Content maxWidth="min(720px, calc(100vw - 24px))" style={{maxHeight:"calc(100dvh - 24px)",overflowY:"auto"}}>
      <Dialog.Title>{t("Mock JSON Schema")}</Dialog.Title>
      <Dialog.Description>{t("保存后预览；支持基本类型、oneOf/anyOf、对象 allOf、可空类型、元组和本地 $defs 引用。静态正文保留。")}</Dialog.Description>
      <Editor label={t("Mock JSON Schema")} value={draft} onChange={setDraft} dark={dark} language="json" height="320px" />
      {invalidSchemaInput(draft) && <Text role="alert" size="1" color="red">{t("Schema 必须是 256 KiB 内的 JSON 对象；保存时会检查支持的规则。")}</Text>}
      <Flex justify="end" mt="4"><Dialog.Close><Button variant="soft" color="gray" disabled={invalidSchemaInput(draft)} onClick={()=>onCommit(draft)}>{t("完成")}</Button></Dialog.Close></Flex>
    </Dialog.Content>
  </Dialog.Root>;
}
