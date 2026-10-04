import { useState } from "react";
import { Button, Flex, Text, TextArea, TextField } from "@radix-ui/themes";
import { PairEditor } from "../../shared/ui";
import { t, useLanguage } from "../../shared/i18n";
import type { Receiver } from "./model";
export function ReceiverSettings({ receiver, busy, save }: { receiver: Receiver; busy: boolean; save: (i: Receiver) => Promise<void> }) {
  useLanguage();
  const [name, setName] = useState(receiver.name);
  const [response, setResponse] = useState(receiver.response);
  return <form className="webhook-settings" onSubmit={e => { e.preventDefault(); void save({ ...receiver, name, response }); }}>
    <Flex gap="3" wrap="wrap" align="end">
      <label><Text as="div" size="2">{t("接收器名称")}</Text><TextField.Root required maxLength={256} value={name} onChange={e => setName(e.target.value)} /></label>
      <label><Text as="div" size="2">{t("回复状态码")}</Text><TextField.Root type="number" required min={200} max={599} value={response.status} onChange={e => setResponse({ ...response, status: Number(e.target.value) })} /></label>
      <Button disabled={busy} type="submit">{t("保存回复配置")}</Button>
    </Flex>
    <Text as="div" size="2">{t("回复请求头")}</Text>
    <PairEditor rows={response.headers} onChange={headers => setResponse({ ...response, headers })} secrets disabled={busy} />
    <label><Text as="div" size="2">{t("回复正文")}</Text><TextArea className="mono" value={response.body} onChange={e => setResponse({ ...response, body: e.target.value })} maxLength={65536} /></label>
  </form>;
}
