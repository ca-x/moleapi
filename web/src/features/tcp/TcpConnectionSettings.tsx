import { Checkbox, Flex, Text, TextField } from "@radix-ui/themes";
import { Choice, Field } from "../../shared/ui";
import { t, useLanguage } from "../../shared/i18n";
import type { RequestSpec } from "../../shared/types";
import type { TcpConfig } from "./types";
import NetworkSettings from "../network/NetworkSettings";

export default function TcpConnectionSettings({ request, scope, onChange, disabled = false }: {
  request: RequestSpec; scope: string; onChange: (patch: Partial<RequestSpec>) => void; disabled?: boolean;
}) {
  useLanguage();
  const config = request.protocol?.kind === "tcp" ? request.protocol : null;
  if (!config) return null;
  const change = (patch: Partial<TcpConfig>) => onChange({ protocol: { ...config, ...patch } });
  return <Flex direction="column" gap="3">
    <NetworkSettings key={scope} rawTcp value={request.network} change={(network) => onChange({ network })} disabled={disabled} />
    <Flex gap="4" wrap="wrap">
      <Field label={t("分帧方式")}><Choice value={config.framing} label={t("TCP 分帧方式")} disabled={disabled} options={[{ value: "raw", label: t("原始字节流") }, { value: "lines", label: t("UTF8 行（LF / CRLF）") }, { value: "length_be", label: t("4 字节长度前缀 · 大端") }, { value: "length_le", label: t("4 字节长度前缀 · 小端") }]} onChange={(framing) => change({ framing: framing as TcpConfig["framing"] })} /></Field>
      <Field label={t("连接超时（ms）")}><TextField.Root type="number" min="1" max="120000" value={request.timeout_ms} disabled={disabled} onChange={(event) => onChange({ timeout_ms: Number(event.target.value) })} /></Field>
      <Field label={t("单帧上限（字节）")}><TextField.Root type="number" min="1" max="1048576" value={config.max_frame_bytes} disabled={disabled} onChange={(event) => change({ max_frame_bytes: Number(event.target.value) })} /></Field>
      <Field label={t("空闲超时（ms，0 为关闭）")}><TextField.Root type="number" min="0" max="120000" value={config.idle_timeout_ms} disabled={disabled} onChange={(event) => change({ idle_timeout_ms: Number(event.target.value) })} /></Field>
    </Flex>
    <Text as="label" size="2"><Flex gap="2" align="center"><Checkbox checked={request.verify_tls === true} disabled={disabled} onCheckedChange={(value) => onChange({ verify_tls: value === true })} />{t("验证 TLS 证书")}</Flex></Text>
    <Text as="label" size="2"><Flex gap="2" align="center"><Checkbox checked={config.no_delay} disabled={disabled} onCheckedChange={(value) => change({ no_delay: value === true })} />{t("禁用 Nagle（TCP_NODELAY）")}</Flex></Text>
    <Text as="p" size="2" color="gray">{t("tcp://host:port 或 tcps://host:port；TLS 证书验证在此处设置。原始模式展示读取块，不代表对端报文边界。行分帧追加 LF 并去除接收的 LF／CRLF；长度分帧由编解码器添加和读取 4 字节前缀。")}</Text>
    <Text as="p" size="2" color="gray">{t("默认导出隐藏私密或无法可靠筛查的二进制草稿；保留完整原文请显式选择包含私密值导出。")}</Text>
    <Text as="p" size="2" color="gray">{t("HTTP 鉴权、请求头、查询参数及响应脚本不用于 TCP。连接前脚本沿用项目、集合和请求设置。")}</Text>
  </Flex>;
}
