import { t, useLanguage } from "../../shared/i18n";
import { useEffect, useMemo, useState } from "react";
import { Checkbox, Flex, Text } from "@radix-ui/themes";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { Choice } from "../../shared/ui";
import type { ProtocolEvent } from "../protocols/types";
import { telemetryFieldLabel, telemetrySamples } from "./telemetry";
const colors = [
  "var(--cyan-11)",
  "var(--amber-11)",
  "var(--green-11)",
  "var(--violet-11)",
];
export default function MqttTelemetry({
  events,
  sessionId,
}: {
  events: ProtocolEvent[];
  sessionId?: string;
}) {
  useLanguage();
  const samples = useMemo(() => telemetrySamples(events), [events]);
  const topics = [...new Set(samples.map((sample) => sample.topic))];
  const [topic, setTopic] = useState("");
  const [fields, setFields] = useState<string[] | null>(null);
  const [style, setStyle] = useState("line");
  const actualTopic = topics.includes(topic) ? topic : topics[0] || "";
  const series = samples.filter((sample) => sample.topic === actualTopic);
  const available = [
    ...new Set(series.flatMap((sample) => Object.keys(sample.values))),
  ].slice(0, 64);
  const selected = fields?.filter((field) => available.includes(field));
  const activeFields = selected ?? available.slice(0, 1);
  useEffect(() => {
    setTopic("");
    setFields(null);
    setStyle("line");
  }, [sessionId]);
  const data = series.map((sample) => ({
    time: sample.time,
    ...Object.fromEntries(
      activeFields.map((field, index) => [
        "value" + index,
        sample.values[field],
      ]),
    ),
  }));
  const formatTime = (value: number) => new Date(value).toLocaleTimeString();
  if (!samples.length)
    return (
      <Text color="gray"> {t("收到 JSON 数字字段后显示遥测图表。私密、二进制及非 JSON 消息不进入图表。")} </Text>
    );
  return (
    <div className="mqtt-telemetry">
      <Flex gap="3" wrap="wrap">
        <Choice
          value={actualTopic}
          label={t("遥测 Topic")}
          onChange={(value) => {
            setTopic(value);
            setFields(null);
          }}
          options={topics.map((value) => ({ value, label: value }))}
        />
        <Choice
          value={style}
          label={t("图表类型")}
          onChange={setStyle}
          options={[
            { value: "line", label: "Line" },
            { value: "bar", label: "Bar" },
          ]}
        />
      </Flex>
      <Flex gap="4" wrap="wrap" aria-label={t("遥测字段")}>
        {available.map((field) => (
          <label className="checkbox-label" key={field}>
            <Checkbox
              checked={activeFields.includes(field)}
              disabled={
                !activeFields.includes(field) && activeFields.length >= 4
              }
              onCheckedChange={(value) =>
                setFields(
                  value === true
                    ? [...new Set([...activeFields, field])].slice(0, 4)
                    : activeFields.filter((entry) => entry !== field),
                )
              }
            />
            {telemetryFieldLabel(field)}
          </label>
        ))}
      </Flex>
      <div
        className="mqtt-chart"
        role="img"
        aria-label={t("MQTT Topic {{topic}} 数字字段的时间序列", { topic: actualTopic })}
      >
        <ResponsiveContainer width="100%" height={300}>
          {style === "bar" ? (
            <BarChart data={data}>
              <CartesianGrid stroke="var(--gray-5)" strokeDasharray="3 3" />
              <XAxis
                dataKey="time"
                tickFormatter={formatTime}
                stroke="var(--gray-11)"
              />
              <YAxis stroke="var(--gray-11)" />
              <Tooltip labelFormatter={(value) => formatTime(Number(value))} />
              {activeFields.map((field, index) => (
                <Bar
                  key={field}
                  dataKey={"value" + index}
                  name={telemetryFieldLabel(field)}
                  fill={colors[index]}
                  isAnimationActive={false}
                />
              ))}
            </BarChart>
          ) : (
            <LineChart data={data}>
              <CartesianGrid stroke="var(--gray-5)" strokeDasharray="3 3" />
              <XAxis
                dataKey="time"
                tickFormatter={formatTime}
                stroke="var(--gray-11)"
              />
              <YAxis stroke="var(--gray-11)" />
              <Tooltip labelFormatter={(value) => formatTime(Number(value))} />
              {activeFields.map((field, index) => (
                <Line
                  key={field}
                  dataKey={"value" + index}
                  name={telemetryFieldLabel(field)}
                  stroke={colors[index]}
                  dot={{ r: 2 }}
                  connectNulls={false}
                  isAnimationActive={false}
                />
              ))}
            </LineChart>
          )}
        </ResponsiveContainer>
      </div>
      <Text size="1" color="gray"> {t("最近 {{count}} 条接收消息；时间为本地接收时间，缺失字段保留为空值。", { count: series.length })} </Text>
    </div>
  );
}
