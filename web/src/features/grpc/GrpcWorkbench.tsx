import { useEffect, useState } from "react";
import { Badge, Button, Callout, Flex, Tabs, Text } from "@radix-ui/themes";
import {
  Braces,
  FileCode,
  RefreshCw,
  Send,
  Square,
  StepForward,
} from "lucide-react";
import { Choice, Editor } from "../../shared/ui";
import { bytes } from "../../shared/model";
import type { GrpcConfig } from "../../shared/types";
import { useWorkbench } from "../workbench/context";
import { useGrpcSchema } from "./useGrpcSchema";
import { methodMode } from "./types";
import ProtoSourceDialog from "./ProtoSourceDialog";
import GrpcEvents from "./GrpcEvents";
export default function GrpcWorkbench() {
  const state = useWorkbench();
  const source = useGrpcSchema();
  const { session, events, dropped, error, busy, sending, send, close } =
    state.protocolSession;
  const [sourceOpen, setSourceOpen] = useState(false);
  const [halfClosedFor, setHalfClosedFor] = useState<string | null>(null);
  const [tab, setTab] = useState("message");
  const identity = JSON.stringify([
    state.accountId,
    state.draft?.id,
    state.request?.id,
    state.draft?.data.active_environment_id,
  ]);
  useEffect(() => {
    setSourceOpen(false);
    setHalfClosedFor(null);
    setTab("message");
  }, [identity]);
  const grpc =
    state.request?.protocol?.kind === "grpc" ? state.request.protocol : null;
  const service = source.schema?.services.find(
    (item) => item.name === grpc?.service,
  );
  const method = service?.methods.find((item) => item.name === grpc?.method);
  const active = !!session && ["connecting", "open"].includes(session.state);
  const halfClosed =
    session?.client_half_closed || (!!session && halfClosedFor === session.id);
  const status = [...events]
    .reverse()
    .find((event) => event.message.kind === "grpc_status")?.message;
  function update(patch: Partial<GrpcConfig>) {
    if (grpc) state.updateRequest({ protocol: { ...grpc, ...patch } });
  }
  async function halfClose() {
    if (!session) return;
    const current = source.guard();
    const id = session.id;
    if ((await send({ kind: "grpc_half_close" })) && current())
      setHalfClosedFor(id);
  }
  if (!grpc) return null;
  return (
    <section className="grpc-workbench" aria-label="gRPC 客户端">
      <div className="grpc-definition-toolbar">
        <Flex gap="3" align="center" wrap="wrap">
          <Choice
            value={state.request?.specification_id || "none"}
            onChange={(value) =>
              state.updateRequest({
                specification_id: value === "none" ? null : value,
                protocol: { ...grpc, service: "", method: "" },
              })
            }
            label="gRPC 服务定义"
            disabled={active}
            options={[
              { value: "none", label: "选择服务定义" },
              ...(state.draft?.data.specifications || [])
                .filter((item) => item.kind === "protobuf")
                .map((item) => ({ value: item.id, label: item.name })),
            ]}
          />
          <Button
            size="1"
            variant="soft"
            color="gray"
            disabled={active}
            onClick={() => setSourceOpen(true)}
          >
            <FileCode size={14} />
            {source.specification ? "Proto 文件" : "导入 Proto"}
          </Button>
          <Button
            size="1"
            variant="soft"
            color="gray"
            loading={source.busy}
            disabled={active || !state.request?.url}
            onClick={() => void source.reflect()}
          >
            <RefreshCw size={14} />
            Server Reflection
          </Button>
        </Flex>
        <Flex gap="3" align="center" wrap="wrap" mt="3">
          <Choice
            value={grpc.service || "none"}
            label="gRPC 服务"
            disabled={active || !source.schema}
            options={[
              { value: "none", label: "选择服务" },
              ...(source.schema?.services || []).map((item) => ({
                value: item.name,
                label: item.name,
              })),
            ]}
            onChange={(value) =>
              update({ service: value === "none" ? "" : value, method: "" })
            }
          />
          <Choice
            value={grpc.method || "none"}
            label="gRPC 方法"
            disabled={active || !service}
            options={[
              { value: "none", label: "选择方法" },
              ...(service?.methods || []).map((item) => ({
                value: item.name,
                label: item.name,
              })),
            ]}
            onChange={(value) =>
              update({ method: value === "none" ? "" : value })
            }
          />
          {method && <Badge color="gray">{methodMode(method)}</Badge>}
          {status?.kind === "grpc_status" && (
            <Badge color={status.code === 0 ? "green" : "red"}>
              {status.code} {status.name}
            </Badge>
          )}
          {session && (
            <Text size="1" color="gray" className="mono">
              ↓ {bytes(session.received_bytes)} · ↑ {bytes(session.sent_bytes)}
            </Text>
          )}
          {(active || busy) && (
            <Button
              size="1"
              variant="soft"
              color="gray"
              onClick={() => void close()}
            >
              <Square size={14} />
              {busy ? "取消调用" : "停止"}
            </Button>
          )}
        </Flex>
      </div>
      {(source.error || error) && (
        <Callout.Root color="red" role="alert">
          <Callout.Text>{source.error || error}</Callout.Text>
        </Callout.Root>
      )}
      {!active && session?.reason && (
        <Text size="1" color="gray" className="grpc-reason">
          {session.reason}
        </Text>
      )}
      <Tabs.Root value={tab} onValueChange={setTab} className="grpc-tabs">
        <Tabs.List>
          <Tabs.Trigger value="message">请求消息</Tabs.Trigger>
          <Tabs.Trigger value="events">
            消息与状态 <span className="count">{events.length}</span>
          </Tabs.Trigger>
          <Tabs.Trigger value="definition">方法定义</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="message">
          <div className="grpc-input-toolbar">
            <Text size="1" color="gray" className="mono">
              {method?.input_type ||
                "选择定义、服务和方法后，输入 Protobuf JSON 请求。"}
            </Text>
            <Flex gap="2" wrap="wrap">
              <Button
                size="1"
                color="gray"
                variant="soft"
                disabled={!method}
                onClick={() =>
                  update({
                    message_source: JSON.stringify(
                      method!.input_template,
                      null,
                      2,
                    ),
                  })
                }
              >
                <Braces size={14} />
                插入模板
              </Button>
              {method?.client_streaming && (
                <>
                  <Button
                    size="1"
                    disabled={session?.state !== "open" || halfClosed}
                    loading={sending}
                    onClick={() =>
                      void send({
                        kind: "grpc_message",
                        message_source: grpc.message_source,
                      })
                    }
                  >
                    <Send size={14} />
                    发送消息
                  </Button>
                  <Button
                    size="1"
                    color="gray"
                    variant="soft"
                    disabled={
                      session?.state !== "open" || halfClosed || sending
                    }
                    onClick={() => void halfClose()}
                  >
                    <StepForward size={14} />
                    {halfClosed ? "发送已结束" : "结束发送"}
                  </Button>
                </>
              )}
            </Flex>
          </div>
          <Editor
            value={grpc.message_source}
            onChange={(message_source) => update({ message_source })}
            dark={state.dark}
            jsonMode
            height="100%"
            label="gRPC 请求消息"
          />
          <Text size="1" color="gray" className="grpc-input-help">
            首次调用发送当前消息；客户端流和双向流可继续发送，结束发送后仍可接收响应。
          </Text>
        </Tabs.Content>
        <Tabs.Content value="events">
          <GrpcEvents
            events={events}
            dropped={dropped}
            dark={state.dark}
            sessionId={session?.id}
          />
        </Tabs.Content>
        <Tabs.Content value="definition">
          {method ? (
            <Editor
              value={JSON.stringify(
                {
                  method,
                  input: source.schema?.messages?.find(
                    (item) => item.name === method.input_type,
                  ),
                  output: source.schema?.messages?.find(
                    (item) => item.name === method.output_type,
                  ),
                  enums: source.schema?.enums,
                },
                null,
                2,
              )}
              dark={state.dark}
              jsonMode
              readOnly
              height="100%"
              label="gRPC 方法定义"
            />
          ) : (
            <Text color="gray">
              导入 Proto 或读取 Server Reflection，然后选择服务与方法。
            </Text>
          )}
        </Tabs.Content>
      </Tabs.Root>
      <ProtoSourceDialog
        open={sourceOpen}
        onOpenChange={setSourceOpen}
        source={source}
      />
    </section>
  );
}
