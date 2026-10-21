import type { ReactNode } from "react";
import { Checkbox, Flex, Text } from "@radix-ui/themes";
import { Choice, Editor } from "../../shared/ui";
import { t, useLanguage } from "../../shared/i18n";
import type { TcpMessage } from "./types";

export default function TcpMessageEditor({ message, onChange, dark, label, height = "100%", disabled = false, children }: {
  message: TcpMessage; onChange: (message: TcpMessage) => void; dark: boolean; label?: string; height?: string; disabled?: boolean; children?: ReactNode;
}) {
  useLanguage();
  return <Flex className="tcp-message-editor" direction="column" gap="3">
    <Flex gap="3" align="center" wrap="wrap">
      <Choice value={message.encoding} label={t("TCP 报文编码")} disabled={disabled} options={[{ value: "text", label: t("UTF8 文本") }, { value: "hex", label: "Hex" }, { value: "base64", label: "Base64" }]} onChange={(encoding) => onChange({ ...message, encoding: encoding as TcpMessage["encoding"] })} />
      <Text as="label" size="2"><Flex gap="2" align="center"><Checkbox disabled={disabled} checked={message.secret} onCheckedChange={(value) => onChange({ ...message, secret: value === true })} />{t("私密报文")}</Flex></Text>
      {children}
    </Flex>
    <Text size="1" color="gray">{t("仅在点击发送时解析本次连接的环境快照；环境修改后重连生效。存在私密变量或发送私密报文后，本次会话的字节载荷会保守隐藏，仍记录方向、大小和状态。")}</Text>
    <Editor value={message.payload_source} onChange={(payload_source) => onChange({ ...message, payload_source })} readOnly={disabled} dark={dark} height={height} label={label ?? t("TCP 报文内容")} />
  </Flex>;
}
