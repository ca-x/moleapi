import { useEffect, useRef, useState } from "react";
import { Button, Callout, Dialog, Flex, Text, TextField } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import { id } from "../../shared/model";
import { resultDatasetMessage } from "../data/resultDatasetMessage";
import type { Result } from "../data/types";
export type SaveSharedResultDataset = (result: Result, name: string, identifier: string, isCurrent: () => boolean) => Promise<void>;
export default function SharedResultDatasetDialog({ result, onClose, onSave }: { result: Result; onClose: () => void; onSave: SaveSharedResultDataset }) {
  useLanguage();
  const [name, setName] = useState(""), [pending, setPending] = useState(false), [problem, setProblem] = useState("");
  const identifier = useRef(id()), mounted = useRef(true), inFlight = useRef(false);
  const latest = useRef({ result, name }); latest.current = { result, name };
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  async function save() {
    if (inFlight.current) return;
    const origin = latest.current;
    const current = () => mounted.current && latest.current.result === origin.result && latest.current.name === origin.name;
    inFlight.current = true; setPending(true); setProblem("");
    try { await onSave(result, name, identifier.current, current); if (current()) onClose(); }
    catch (cause) { if (current()) setProblem(resultDatasetMessage(cause)); }
    finally { inFlight.current = false; if (mounted.current) setPending(false); }
  }
  return <Dialog.Root open onOpenChange={(open) => { if (!open && !inFlight.current) onClose(); }}><Dialog.Content maxWidth="min(480px, calc(100vw - 24px))">
    <Dialog.Title>{t("保存查询结果为数据集")}</Dialog.Title><Dialog.Description>{t("保存本次查询返回的全部行。整数与小数按字符串保存以保持精度；数据集默认为私密。")}</Dialog.Description>
    <Text size="2" color="gray">{t("{{rows}} 行 · {{columns}} 列", { rows: result.rows.length, columns: result.columns.length })}</Text>
    <TextField.Root aria-label={t("数据集名称")} value={name} disabled={pending} maxLength={256} onChange={(event) => setName(event.target.value)} />
    {problem && <Callout.Root color="red" role="alert"><Callout.Text>{problem}</Callout.Text></Callout.Root>}
    <Flex justify="end" gap="2" mt="3"><Button variant="soft" color="gray" disabled={pending} onClick={onClose}>{t("取消")}</Button><Button disabled={pending || !name.trim()} loading={pending} onClick={() => void save()}>{t("保存数据集")}</Button></Flex>
  </Dialog.Content></Dialog.Root>;
}
