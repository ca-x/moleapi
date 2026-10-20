import { useEffect, useRef, useState } from "react";
import { Button, Dialog, Flex, Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { ProtocolSession, SendMessage } from "../protocols/types";
import type { TcpMessage } from "../tcp/types";
import TcpMessageEditor from "../tcp/TcpMessageEditor";

export default function SharedTcpSessionControls(props: { session: ProtocolSession; initial: TcpMessage; dark: boolean; sending: boolean; isCurrent: () => boolean; send: (message: SendMessage) => Promise<boolean> }) {
  useLanguage();
  const [message, setMessage] = useState(() => structuredClone(props.initial));
  const [confirm, setConfirm] = useState<string | null>(null);
  const latest = useRef(props); latest.current = props;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const canSend = () => mounted.current && latest.current.isCurrent() && latest.current.session.state === "open" && !latest.current.session.client_half_closed && !latest.current.sending;
  const disabled = props.session.state !== "open" || !!props.session.client_half_closed || props.sending;
  async function transmit() { if (canSend()) await props.send({ kind: "tcp_send", message: structuredClone(message) }); }
  async function halfClose() {
    if (confirm !== props.session.id || !canSend()) return;
    setConfirm(null);
    await props.send({ kind: "tcp_half_close" });
  }
  return <section aria-label={t("共享 TCP 会话报文")}><Flex direction="column" gap="3">
    <Text size="1" color="gray">{t("此处报文仅用于当前连接，不会自动保存到共享请求。")}</Text>
    {props.session.client_half_closed && <Text role="status">{t("发送端已半关闭")}</Text>}
    <TcpMessageEditor message={message} onChange={(value) => { if (canSend()) setMessage(value); }} dark={props.dark} disabled={disabled} height="180px" label={t("TCP 临时报文")}>
      <Button disabled={disabled} loading={props.sending} onClick={() => void transmit()}>{t("发送报文")}</Button>
      <Button disabled={disabled} variant="soft" onClick={() => { if (canSend()) setConfirm(props.session.id); }}>{t("半关闭")}</Button>
    </TcpMessageEditor>
  </Flex><Dialog.Root open={confirm !== null} onOpenChange={(open) => { if (!open) setConfirm(null); }}><Dialog.Content maxWidth="min(520px, calc(100vw - 24px))">
    <Dialog.Title>{t("半关闭 TCP 发送端")}</Dialog.Title><Dialog.Description>{t("发送 EOF 后继续接收对端数据。本次连接不能再发送报文。")}</Dialog.Description>
    <Text as="p" size="1" className="wrap-anywhere" mt="3">{props.session.url}</Text>
    <Flex justify="end" gap="3" mt="4"><Button variant="soft" onClick={() => setConfirm(null)}>{t("取消")}</Button><Button disabled={disabled} onClick={() => void halfClose()}>{t("确认半关闭")}</Button></Flex>
  </Dialog.Content></Dialog.Root></section>;
}
