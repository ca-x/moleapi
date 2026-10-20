import { Table, Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { ProtocolSession } from "./types";

export default function SessionHandshake({ session }: { session: ProtocolSession }) {
  useLanguage();
  return <>
    <Text as="p" size="1" color="gray" className="wrap-anywhere">{session.url}</Text>
    <Text as="p" size="2" mt="3">{t("状态")} {session.handshake?.status ?? t("等待握手")}</Text>
    <Table.Root><Table.Header><Table.Row><Table.ColumnHeaderCell>{t("名称")}</Table.ColumnHeaderCell><Table.ColumnHeaderCell>{t("值")}</Table.ColumnHeaderCell></Table.Row></Table.Header><Table.Body>
      {session.handshake?.headers.map((header, index) => <Table.Row key={index}><Table.Cell className="mono wrap-anywhere">{header.key}</Table.Cell><Table.Cell className="mono wrap-anywhere">{header.value}</Table.Cell></Table.Row>)}
    </Table.Body></Table.Root>
  </>;
}
