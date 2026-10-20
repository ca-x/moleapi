import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Callout, Checkbox, Flex, Heading, Tabs, Text, TextArea, TextField } from "@radix-ui/themes";
import { api, previewSharedMock, token } from "../../shared/api";
import { t, useLanguage } from "../../shared/i18n";
import { liveError } from "../../shared/i18n/errors";
import { id, newRequest } from "../../shared/model";
import type { ApiResponse, Example, ExportResult, HistoryEntry, MarkdownDocument, Pair, RequestSpec, RunReportPage, SavedDataset, Scenario, Specification, SharedWorkspaceSummary, Workspace, WorkspaceData } from "../../shared/types";
import { Choice, Editor, Field, PairEditor } from "../../shared/ui";
import RequestAuthEditor from "../authentication/RequestAuthEditor";
import { authenticationEligibility } from "../authentication/eligibility";
import RequestBodyEditor from "../request-body/RequestBodyEditor";
import { bodyModePatch } from "../request-body/model";
import RequestScripts from "../scripts/RequestScripts";
import AssertionsEditor from "../testing/AssertionsEditor";
import ExtractionsEditor from "../testing/ExtractionsEditor";
import NetworkSettings from "../network/NetworkSettings";
import { validateMarkdownDocuments } from "../documents/validation";
import { saveProjectFile } from "../generation/saveProjectFile";
import { hasChanges } from "./draft";
import SharedDocumentsPanel from "./SharedDocumentsPanel";
import SharedVariablesPanel from "./SharedVariablesPanel";
import SharedLiveSessionPanel from "./SharedLiveSessionPanel";
import MockTrafficControls from "../requests/MockTrafficControls";
import SharedMockSchemaControls from "./SharedMockSchemaControls";
import MockDataEditor from "../requests/MockDataEditor";
import MockMatchingEditor from "../requests/MockMatchingEditor";
import MockScriptEditor from "../requests/MockScriptEditor";
import { mockRouteInfo } from "../requests/mockRouteInfo";
import SharedCollectionSettingsDialog from "./SharedCollectionSettingsDialog";
import SharedResourceActions from "./SharedResourceActions";
import SharedScenariosPanel from "./SharedScenariosPanel";
import SharedGraphQLWorkbench from "./SharedGraphQLWorkbench";
import SharedSpecificationsPanel from "./SharedSpecificationsPanel";
import SharedRequestSpecification from "./SharedRequestSpecification";
import SharedDatasetsPanel from "./SharedDatasetsPanel";
import SharedRunOptions from "./SharedRunOptions";
import { defaultSharedRunConfig, resolveSharedRunOptions, type SharedRunConfig } from "./sharedRunOptions";
import SharedDataRequestEditor from "./SharedDataRequestEditor";
import { resultDatasetSnapshot } from "../data/dataset";
import type { Result as DataResult } from "../data/types";
import { dataConfig } from "../data/model";
import type { DatasetPreview } from "../testing/RunnerDataset";
import type { CollectionSettingsTarget } from "../collections/CollectionSettingsFields";

type Tab = "resources" | "history" | "reports" | "mock" | "documents" | "variables" | "scenarios" | "datasets" | "specifications";
type RunResult = { passed: number; failed: number; cancelled: boolean; report_id?: string };
type MockCall = { id: string; request_id: string | null; example_id: string | null; response_status: number; outcome: string; at: string };

export default function SharedWorkspaceView({ accountId, selected, dark, onClose, onDirtyChange }: { accountId: string; selected: SharedWorkspaceSummary; dark: boolean; onClose: () => void; onDirtyChange: (dirty: boolean) => void }) {
  const { language } = useLanguage();
  const client = useQueryClient();
  const credential = token();
  const boundary = useRef({ accountId, credential, epoch: 0 });
  if (boundary.current.accountId !== accountId || boundary.current.credential !== credential) boundary.current = { accountId, credential, epoch: boundary.current.epoch + 1 };
  const epoch = boundary.current.epoch;
  const owner = selected.owner_id;
  const workspaceId = selected.workspace_id;
  const base = `/api/owners/${encodeURIComponent(owner)}/workspaces/${encodeURIComponent(workspaceId)}`;
  const [draft, setDraft] = useState<Workspace | null>(null);
  const [saved, setSaved] = useState<Workspace | null>(null);
  const [loadedScope, setLoadedScope] = useState("");
  const [conflict, setConflict] = useState<Workspace | null>(null);
  const [tab, setTab] = useState<Tab>("resources");
  const [requestEditorTab, setRequestEditorTab] = useState("query");
  const [collectionId, setCollectionId] = useState("");
  const [collectionSettings, setCollectionSettings] = useState<CollectionSettingsTarget | null>(null);
  const [requestId, setRequestId] = useState("");
  const [exampleId, setExampleId] = useState("");
  const [exampleStatusInput, setExampleStatusInput] = useState("");
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState(false);
  const [stoppingRun, setStoppingRun] = useState(false);
  const activeRun = useRef<{ jobId: string; scope: string; stopping: boolean; label: string } | null>(null);
  const runGeneration = useRef(0);
  const [sending, setSending] = useState(false);
  const sendGeneration = useRef(0);
  const [requestResponse, setRequestResponse] = useState<ApiResponse | null>(null);
  const [responseSource, setResponseSource] = useState<"saved" | "draft" | null>(null);
  const [mocking, setMocking] = useState(false);
  const [mockPreview, setMockPreview] = useState<{ status: number; body: string } | null>(null);
  const mockGeneration = useRef(0);
  const linkedSchemaGeneration = useRef(0);
  const [linkingSchema, setLinkingSchema] = useState(false);
  const [error, setError] = useState<ReturnType<typeof liveError>>("");
  const [runResult, setRunResult] = useState<RunResult | null>(null);
  const [reportFormat, setReportFormat] = useState("html");
  const [exportingReport, setExportingReport] = useState<string | null>(null);
  const mounted = useRef(true);
  const scope = `${accountId}:${owner}:${workspaceId}:${selected.role}:${epoch}`;
  const [runConfigState, setRunConfigState] = useState<{ scope: string; value: SharedRunConfig } | null>(null);
  const [runDataBusy, setRunDataBusy] = useState<{ scope: string; busy: boolean } | null>(null);
  const runConfig = runConfigState?.scope === scope ? runConfigState.value : defaultSharedRunConfig();
  const dataBusy = runDataBusy?.scope === scope && runDataBusy.busy;
  const runOptionsValid = !dataBusy && resolveSharedRunOptions(runConfig, saved?.data.datasets ?? []).options !== null;
  useEffect(() => { setRunConfigState(null); setRunDataBusy(null); }, [scope]);
  const latest = useRef(scope);
  latest.current = scope;
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  useEffect(() => { activeRun.current = null; setStoppingRun(false); }, [scope]);
  useEffect(() => { setDraft(null); setSaved(null); setLoadedScope(""); setConflict(null); setCollectionId(""); setRequestId(""); setExampleId(""); setTab("resources"); setRequestEditorTab("query"); setError(""); setRunResult(null); setReportFormat("html"); setExportingReport(null); setRequestResponse(null); setResponseSource(null); setMockPreview(null); setSaving(false); setRunning(false); setSending(false); setMocking(false); }, [scope]);
  const current = (origin: string) => mounted.current && latest.current === origin && token() === credential;
  const workspace = useQuery({
    queryKey: ["shared-workspace", accountId, owner, workspaceId, selected.role, epoch],
    queryFn: () => api<Workspace>(base), enabled: !!credential, retry: false, refetchInterval: 3000,
  });
  const history = useQuery({ queryKey: ["shared-history", accountId, owner, workspaceId, epoch], queryFn: () => api<HistoryEntry[]>(`${base}/history`), enabled: !!credential && selected.role !== "viewer" && tab === "history" && !workspace.error, retry: false });
  const reports = useQuery({ queryKey: ["shared-reports", accountId, owner, workspaceId, epoch], queryFn: () => api<RunReportPage>(`${base}/reports`), enabled: !!credential && selected.role !== "viewer" && tab === "reports" && !workspace.error, retry: false });
  const mockCalls = useQuery({ queryKey: ["shared-mock-calls", accountId, owner, workspaceId, epoch], queryFn: () => api<MockCall[]>(`${base}/mock-calls`), enabled: !!credential && selected.role !== "viewer" && tab === "mock" && !workspace.error, retry: false });
  useEffect(() => {
    if (workspace.error || !workspace.data) { if (workspace.error) { sendGeneration.current += 1; runGeneration.current += 1; mockGeneration.current += 1; setSending(false); setRunning(activeRun.current?.scope === scope); setMocking(false); setDraft(null); setSaved(null); setLoadedScope(""); setRunResult(null); setRequestResponse(null); setResponseSource(null); setMockPreview(null); } return; }
    if (!saved) { setDraft(workspace.data); setSaved(workspace.data); setLoadedScope(scope); return; }
    if (workspace.data.revision !== saved.revision) {
      if (hasChanges({ draft, saved })) { invalidateDraftResults(); setConflict(workspace.data); }
      else { invalidateDraftResults(); setDraft(workspace.data); setSaved(workspace.data); setConflict(null); }
    }
  }, [workspace.data, workspace.error, draft, saved, scope]);
  const canEdit = selected.role !== "viewer";
  useEffect(() => { setCollectionSettings(null); }, [scope, saved?.revision, conflict?.revision]);
  const dirty = hasChanges({ draft, saved });
  const documentValidation = validateMarkdownDocuments(draft?.data.documents);
  const invalidDocuments = !!(documentValidation.invalidTitleDocument || documentValidation.invalidSourceDocument || documentValidation.invalidTotalSource);
  const documentError = documentValidation.invalidTitleDocument
    ? t("文档「{{value0}}」的标题须为 1–128 个字符，不能含首尾空格或控制字符。", { value0: documentValidation.invalidTitleDocument.name })
    : documentValidation.invalidSourceDocument
      ? t("文档「{{value0}}」的 Markdown 正文不能超过 256 KiB。", { value0: documentValidation.invalidSourceDocument.name })
      : documentValidation.invalidTotalSource ? t("工作区 Markdown 正文合计不能超过 2 MiB。") : "";
  useEffect(() => { onDirtyChange(dirty); }, [dirty, onDirtyChange]);
  useEffect(() => () => onDirtyChange(false), [onDirtyChange]);
  const collection = draft?.data.collections.find((item) => item.id === collectionId) ?? draft?.data.collections[0];
  const request = collection?.requests.find((item) => item.id === requestId) ?? collection?.requests[0];
  const selectedRequestRef = useRef(request?.id); selectedRequestRef.current = request?.id;
  const graphqlRequest = request?.protocol?.kind === "graphql";
  const httpRequest = !!request && (!request.protocol?.kind || request.protocol.kind === "http");
  const matchRoutePath = request ? mockRouteInfo(request).path : "";
  const httpLiveRequest = request?.protocol?.kind === "sse" || request?.protocol?.kind === "websocket";
  const websocketRequest = request?.protocol?.kind === "websocket";
  const liveRequest = request?.protocol?.kind === "sse" || request?.protocol?.kind === "websocket" || request?.protocol?.kind === "data";
  const example = request?.examples.find((item) => item.id === exampleId) ?? request?.examples[0];
  useEffect(() => { linkedSchemaGeneration.current += 1; setLinkingSchema(false); }, [scope, collection?.id, request?.id, example?.id, tab]);
  const statusPending = !!example && exampleStatusInput !== String(example.status);
  useEffect(() => { setExampleStatusInput(example ? String(example.status) : ""); }, [collection?.id, request?.id, example?.id, example?.status]);
  useEffect(() => { runGeneration.current += 1; setRunning(activeRun.current?.scope === scope); setRunResult(null); }, [collection?.id]);
  useEffect(() => { sendGeneration.current += 1; setSending(false); setRequestResponse(null); setResponseSource(null); }, [collection?.id, request?.id]);
  useEffect(() => { mockGeneration.current += 1; setMocking(false); setMockPreview(null); }, [collection?.id, request?.id, example?.id]);
  function invalidateDraftResults() {
    linkedSchemaGeneration.current += 1;
    setLinkingSchema(false);
    sendGeneration.current += 1;
    setSending(false);
    setRequestResponse(null);
    setResponseSource(null);
    runGeneration.current += 1;
    setRunning(activeRun.current?.scope === scope);
    setRunResult(null);
    mockGeneration.current += 1;
    setMocking(false);
    setMockPreview(null);
  }
  function updateRequest(change: (request: RequestSpec) => RequestSpec) {
    if (!canEdit || !collection || !request) return;
    invalidateDraftResults();
    setDraft((currentDraft) => currentDraft ? ({ ...currentDraft, data: { ...currentDraft.data, collections: currentDraft.data.collections.map((item) => item.id === collection.id ? { ...item, requests: item.requests.map((entry) => entry.id === request.id ? change(entry) : entry) } : item) } }) : null);
  }
  function updateSelectedRequest(patch: Partial<RequestSpec>) {
    updateRequest((item) => ({ ...item, ...patch }));
  }
  function applyResourceChange(data: WorkspaceData, selectedCollection: string, selectedRequest: string) {
    if (!canEdit || !current(scope)) return;
    invalidateDraftResults();
    setDraft((currentDraft) => currentDraft ? { ...currentDraft, data } : null);
    setCollectionId(selectedCollection); setRequestId(selectedRequest); setExampleId("");
  }
  function applyCollectionSettings(data: WorkspaceData, selectedId: string) {
    if (!canEdit || !current(scope)) return;
    invalidateDraftResults();
    setDraft((currentDraft) => currentDraft ? { ...currentDraft, data } : null);
    setCollectionId(selectedId); setRequestId("");
  }
  function addHttpRequest(kind: "http" | "data" | "graphql" | "sse" | "websocket" = "http") {
    if (!canEdit || !collection) return;
    const next = newRequest();
    if (kind === "data") { next.protocol = dataConfig(); next.name = t("数据查询"); next.url = ""; }
    if (kind === "graphql") { next.protocol = { kind: "graphql", document: "query Query { __typename }", variables: {}, variables_source: "{}", operation_name: null, connection_params: {}, subscription_url: null }; next.method = "POST"; next.name = "GraphQL"; next.url = ""; }
    if (kind === "sse" || kind === "websocket") { next.protocol = { kind }; next.method = "GET"; next.name = kind === "sse" ? "SSE" : "WebSocket"; next.url = ""; }
    invalidateDraftResults();
    setDraft((currentDraft) => currentDraft ? ({ ...currentDraft, data: { ...currentDraft.data, collections: currentDraft.data.collections.map((item) => item.id === collection.id ? { ...item, requests: [...item.requests, next] } : item) } }) : null);
    setRequestId(next.id);
  }
  function updateExample(change: (example: Example) => Example) {
    if (!example) return;
    updateRequest((item) => ({ ...item, examples: item.examples.map((entry) => entry.id === example.id ? change(entry) : entry) }));
  }
  function addExample() {
    if (!request) return;
    const nextId = id();
    updateRequest((item) => ({ ...item, examples: [...item.examples, { id: nextId, name: t("新示例"), status: 200, headers: [], body: "" }] }));
    setExampleId(nextId);
  }
  function updateDocuments(documents: MarkdownDocument[]) {
    if (!canEdit) return;
    invalidateDraftResults();
    setDraft((currentDraft) => currentDraft ? ({ ...currentDraft, data: { ...currentDraft.data, documents } }) : null);
  }
  function datasetContextCurrent() {
    const cached = client.getQueryState<Workspace>(["shared-workspace", accountId, owner, workspaceId, selected.role, epoch]);
    return current(scope) && cached?.status === "success" && cached.data?.revision === saved?.revision;
  }
  function attachGraphQLSchema(specification: Specification) {
    if (!canEdit || !draft || !request || selectedRequestRef.current !== request.id || request.protocol?.kind !== "graphql" || dirty || conflict || !datasetContextCurrent()) return;
    const existing = draft.data.specifications?.find((spec) => spec.kind === specification.kind && spec.source === specification.source);
    const selectedSpec = existing ?? specification, targetId = request.id;
    if (!["graphql-sdl", "graphql-introspection"].includes(selectedSpec.kind)) return;
    invalidateDraftResults();
    setDraft((now) => now ? { ...now, data: { ...now.data,
      specifications: existing ? now.data.specifications : [...(now.data.specifications ?? []), selectedSpec],
      collections: now.data.collections.map((item) => ({ ...item, requests: item.requests.map((entry) => entry.id === targetId ? { ...entry, specification_id: selectedSpec.id } : entry) })),
    } } : null);
    return selectedSpec.id;
  }
  function updateSpecificationData(data: WorkspaceData) {
    if (!canEdit || !current(scope) || !datasetContextCurrent() || conflict) return;
    invalidateDraftResults(); setDraft((now) => now ? { ...now, data } : null);
  }
  function updateDatasets(datasets: SavedDataset[]) {
    if (!canEdit || !current(scope)) return;
    invalidateDraftResults();
    setDraft((now) => now ? { ...now, data: { ...now.data, datasets } } : null);
  }
  function updateScenarios(scenarios: Scenario[]) {
    if (!canEdit || running || !current(scope)) return;
    invalidateDraftResults();
    setDraft((currentDraft) => currentDraft ? { ...currentDraft, data: { ...currentDraft.data, scenarios } } : null);
  }
  function updateVariables(data: WorkspaceData) {
    if (!canEdit) return;
    const shared = (rows: Pair[]) => rows.map((row) => { const next = { ...row }; delete next.local_value; return next; });
    invalidateDraftResults();
    setDraft((currentDraft) => currentDraft ? ({ ...currentDraft, data: {
      ...data,
      global_variables: data.global_variables ? shared(data.global_variables) : data.global_variables,
      collections: data.collections.map((item) => ({ ...item, variables: item.variables ? shared(item.variables) : item.variables })),
      environments: data.environments.map((item) => ({ ...item, variables: shared(item.variables) })),
    } }) : null);
  }
  function openSharedRequest(requestId: string) {
    const target = draft?.data.collections.find((item) => item.requests.some((request) => request.id === requestId));
    if (!target) return;
    setCollectionId(target.id);
    setRequestId(requestId);
    setTab("resources");
  }
  async function save() {
    if (!canEdit || !draft || !saved || !dirty || saving || conflict || invalidDocuments || !current(scope)) return;
    const origin = scope;
    const snapshot = structuredClone(draft);
    setSaving(true); setError("");
    try {
      const updated = await api<Workspace>(base, "PUT", { name: snapshot.name, data: snapshot.data, expected_revision: saved.revision });
      if (current(origin)) { invalidateDraftResults(); setDraft((latestDraft) => {
        if (latestDraft?.id !== updated.id) return latestDraft;
        if (!hasChanges({ draft: latestDraft, saved: snapshot })) return updated;
        return { ...updated, name: latestDraft.name, data: latestDraft.data };
      }); setSaved(updated); setConflict(null); client.setQueryData(["shared-workspace", accountId, owner, workspaceId, selected.role, epoch], updated); void client.invalidateQueries({ queryKey: ["shared-workspaces", accountId] }); }
    } catch (cause) {
      if (current(origin)) { setError(liveError(cause)); try { const remote = await api<Workspace>(base); if (current(origin) && remote.revision !== saved.revision) setConflict(remote); } catch { /* Access revocation is shown by the next workspace refresh. */ } }
    } finally { if (current(origin)) setSaving(false); }
  }
  async function saveResultDataset(result: DataResult, name: string, identifier: string, resultCurrent: () => boolean) {
    const admitted = () => canEdit && !!saved && !!draft && !dirty && !saving && !conflict && datasetContextCurrent() && resultCurrent();
    if (!admitted() || !saved) throw new Error("stale result context");
    const origin = scope, snapshot = structuredClone(saved);
    if (snapshot.data.datasets?.some((item) => item.id === identifier)) throw new Error("dataset already exists");
    const dataset = resultDatasetSnapshot(result, name, snapshot.data.datasets ?? [], identifier);
    setSaving(true);
    try {
      await api(`${base}/dataset/preview`, "POST", { expected_revision: snapshot.revision, dataset: dataset.source });
      if (!admitted()) throw new Error("stale result context");
      const data = { ...snapshot.data, datasets: [...(snapshot.data.datasets ?? []), dataset] };
      const updated = await api<Workspace>(base, "PUT", { name: snapshot.name, data, expected_revision: snapshot.revision });
      const persisted = updated.data.datasets?.find((item) => item.id === identifier);
      if (updated.id !== snapshot.id || updated.revision <= snapshot.revision || persisted?.name !== dataset.name || !persisted.secret || persisted.source?.format !== dataset.source?.format || persisted.source?.source !== dataset.source?.source) throw new Error("dataset was not persisted");
      if (current(origin)) {
        invalidateDraftResults();
        setDraft((latestDraft) => {
          if (!latestDraft || latestDraft.id !== updated.id) return latestDraft;
          if (!hasChanges({ draft: latestDraft, saved: snapshot })) return updated;
          const datasets = latestDraft.data.datasets ?? [];
          return { ...updated, name: latestDraft.name, data: { ...latestDraft.data, datasets: datasets.some((item) => item.id === identifier) ? datasets : [...datasets, dataset] } };
        });
        setSaved(updated); setConflict(null);
        client.setQueryData(["shared-workspace", accountId, owner, workspaceId, selected.role, epoch], updated);
        void client.invalidateQueries({ queryKey: ["shared-workspaces", accountId] });
      }
    } finally { if (current(origin)) setSaving(false); }
  }
  async function previewRunData(source: { format: "csv" | "json"; source: string }) {
    if (!saved || !datasetContextCurrent()) throw new Error("stale dataset context");
    const result = await api<DatasetPreview>(`${base}/dataset/preview`, "POST", { expected_revision: saved.revision, dataset: source });
    if (!datasetContextCurrent()) throw new Error("stale dataset context");
    return result;
  }
  async function runCollection(scenarioId?: string) {
    if (!canEdit || !collection || running || activeRun.current?.scope === scope || dirty || dataBusy || !datasetContextCurrent()) return;
    const options = resolveSharedRunOptions(runConfig, saved?.data.datasets ?? []).options;
    if (!options) return;
    const scenario = scenarioId ? saved?.data.scenarios?.find((scenario) => scenario.id === scenarioId && scenario.collection_id === collection.id) : undefined;
    if (scenarioId && !scenario) return;
    const origin = scope;
    const generation = ++runGeneration.current;
    const jobId = id();
    const job = { jobId, scope: origin, stopping: false, label: scenario ? `${collection.name} / ${scenario.name}` : collection.name };
    activeRun.current = job;
    setRunning(true); setError(""); setRunResult(null);
    try {
      const result = await api<RunResult>(`${base}/run`, "POST", { ...options, collection_id: collection.id, ...(scenarioId ? { scenario_id: scenarioId } : {}), job_id: jobId });
      if (current(origin)) { if (runGeneration.current === generation) setRunResult(result); void client.invalidateQueries({ queryKey: ["shared-history", accountId, owner, workspaceId] }); void client.invalidateQueries({ queryKey: ["shared-reports", accountId, owner, workspaceId] }); }
    } catch (cause) { if (current(origin) && runGeneration.current === generation) setError(liveError(cause)); }
    finally { if (current(origin) && activeRun.current === job) { activeRun.current = null; setRunning(false); setStoppingRun(false); } }
  }
  async function stopCollection() {
    const job = activeRun.current;
    if (!job || job.scope !== scope || job.stopping || !current(scope)) return;
    job.stopping = true;
    setStoppingRun(true);
    setError("");
    try { await api(`${base}/run/cancel`, "POST", { job_id: job.jobId }); }
    catch (cause) { if (current(job.scope) && activeRun.current === job) setError(liveError(cause)); }
    finally { if (current(job.scope) && activeRun.current === job) { job.stopping = false; setStoppingRun(false); } }
  }
  async function downloadReport(reportId: string) {
    if (!canEdit || exportingReport || !current(scope)) return;
    const origin = scope;
    setExportingReport(reportId);
    setError("");
    try {
      const file = await api<ExportResult>(`${base}/reports/${encodeURIComponent(reportId)}/export?format=${encodeURIComponent(reportFormat)}&language=${encodeURIComponent(language)}`);
      if (current(origin)) await saveProjectFile(file.filename, new TextEncoder().encode(file.content), () => current(origin));
    } catch (cause) { if (current(origin)) setError(liveError(cause)); }
    finally { if (current(origin)) setExportingReport(null); }
  }
  async function sendSavedRequest() {
    if (!canEdit || !request || sending || dirty || !current(scope)) return;
    const origin = scope;
    const requestId = request.id;
    const generation = ++sendGeneration.current;
    setSending(true); setError(""); setRequestResponse(null);
    try {
      const response = await api<ApiResponse>(`${base}/requests/${encodeURIComponent(requestId)}/execute`, "POST", {});
      if (current(origin) && sendGeneration.current === generation) { setRequestResponse(response); setResponseSource("saved"); void client.invalidateQueries({ queryKey: ["shared-history", accountId, owner, workspaceId] }); }
    } catch (cause) { if (current(origin) && sendGeneration.current === generation) setError(liveError(cause)); }
    finally { if (current(origin) && sendGeneration.current === generation) setSending(false); }
  }
  async function sendDraftRequest() {
    if (!canEdit || !httpRequest || !request || !collection || !saved || !dirty || sending || saving || conflict || !current(scope)) return;
    const origin = scope;
    const generation = ++sendGeneration.current;
    const snapshot = structuredClone(request);
    setSending(true); setError(""); setRequestResponse(null); setResponseSource(null);
    try {
      const response = await api<ApiResponse>(`${base}/execute-draft`, "POST", { request: snapshot, collection_id: collection.id, expected_workspace_revision: saved.revision });
      if (current(origin) && sendGeneration.current === generation) { setRequestResponse(response); setResponseSource("draft"); void client.invalidateQueries({ queryKey: ["shared-history", accountId, owner, workspaceId] }); }
    } catch (cause) { if (current(origin) && sendGeneration.current === generation) setError(liveError(cause)); }
    finally { if (current(origin) && sendGeneration.current === generation) setSending(false); }
  }
  async function selectLinkedSchema(pointer: string) {
    if (!canEdit || !httpRequest || !request || !example || !saved || !current(scope)) return;
    if (dirty || saving || conflict || statusPending) { setError(t("先保存工作区，再选择关联响应。")); return; }
    const origin = scope;
    const generation = ++linkedSchemaGeneration.current;
    setLinkingSchema(true); setError("");
    try {
      const result = await api<{ method: string; status: number; media_type: string }>(`${base}/mock-schema/validate`, "POST", { request_id: request.id, specification_id: request.specification_id, pointer, expected_revision: saved.revision });
      if (current(origin) && datasetContextCurrent() && linkedSchemaGeneration.current === generation) updateExample((item) => ({ ...item, status: result.status, headers: item.headers.map((header) => header.enabled && header.key.toLowerCase() === "content-type" ? { ...header, value: result.media_type } : header), mock_schema: undefined, mock_data: undefined, mock_schema_link: { specification_id: request.specification_id!, pointer } }));
    } catch (cause) { if (current(origin) && linkedSchemaGeneration.current === generation) setError(liveError(cause)); }
    finally { if (current(origin) && linkedSchemaGeneration.current === generation) setLinkingSchema(false); }
  }
  async function previewExample() {
    if (!canEdit || !request || !example || !!example.mock_data?.parameters?.length || mocking || dirty || statusPending || conflict || !current(scope)) return;
    const origin = scope;
    const generation = ++mockGeneration.current;
    setMocking(true); setError(""); setMockPreview(null);
    try {
      const result = await previewSharedMock(`${base}/mock/${encodeURIComponent(request.id)}/${encodeURIComponent(example.id)}`);
      if (current(origin) && mockGeneration.current === generation) { setMockPreview(result); void client.invalidateQueries({ queryKey: ["shared-mock-calls", accountId, owner, workspaceId] }); }
    } catch (cause) { if (current(origin) && mockGeneration.current === generation) setError(liveError(cause)); }
    finally { if (current(origin) && mockGeneration.current === generation) setMocking(false); }
  }
  return <main className="main-workspace shared-workspace" aria-label={t("共享工作区")}>
    <Flex align="center" justify="between" gap="3" wrap="wrap"><Flex direction="column" gap="1"><Heading size="5">{selected.workspace_name}</Heading><Text size="2" color="gray">{t("所有者")}: {selected.owner_username} · {selected.role === "viewer" ? t("查看者") : selected.role === "admin" ? t("管理员") : t("编辑者")}</Text></Flex><Button variant="soft" color="gray" onClick={onClose}>{t("返回我的工作区")}</Button></Flex>
    {selected.role === "viewer" && <Callout.Root color="gray"><Callout.Text>{t("此工作区为只读。不能保存、运行请求或查看私有执行记录。")}</Callout.Text></Callout.Root>}
    {workspace.isPending && <Text role="status">{t("正在加载共享工作区…")}</Text>}
    {workspace.error && <Callout.Root color="red" role="alert"><Callout.Text>{t("共享工作区访问已失效或暂时不可用。")}</Callout.Text></Callout.Root>}
    {error && <Callout.Root color="red" role="alert"><Callout.Text>{error}</Callout.Text></Callout.Root>}
    {documentError && <Callout.Root color="red" role="alert"><Callout.Text>{documentError}</Callout.Text></Callout.Root>}
    {!workspace.error && draft && loadedScope === scope && <Flex direction="column" gap="4">
      {canEdit && running && activeRun.current?.scope === scope && <Flex role="region" aria-label={t("当前运行任务")} gap="2" justify="between" align="center" wrap="wrap"><Text size="2" style={{ overflowWrap: "anywhere", minWidth: 0 }}>{t("正在运行：{{name}}", { name: activeRun.current.label })}</Text><Button variant="outline" color="gray" loading={stoppingRun} disabled={stoppingRun} onClick={() => void stopCollection()}>{t("停止集合运行")}</Button></Flex>}
      <Flex gap="2" wrap="wrap"><Button variant={tab === "resources" ? "solid" : "outline"} onClick={() => setTab("resources")}>{t("资源")}</Button><Button variant={tab === "documents" ? "solid" : "outline"} onClick={() => setTab("documents")}>{t("文档")}</Button><Button variant={tab === "specifications" ? "solid" : "outline"} onClick={() => setTab("specifications")}>{t("API 规范")}</Button><Button variant={tab === "variables" ? "solid" : "outline"} onClick={() => setTab("variables")}>{t("环境变量")}</Button>{canEdit && <><Button variant={tab === "datasets" ? "solid" : "outline"} onClick={() => setTab("datasets")}>{t("工作区数据集")}</Button><Button variant={tab === "scenarios" ? "solid" : "outline"} onClick={() => setTab("scenarios")}>{t("工作区测试场景")}</Button><Button variant={tab === "history" ? "solid" : "outline"} onClick={() => setTab("history")}>{t("执行历史")}</Button><Button variant={tab === "reports" ? "solid" : "outline"} onClick={() => setTab("reports")}>{t("测试报告")}</Button><Button variant={tab === "mock" ? "solid" : "outline"} onClick={() => setTab("mock")}>{t("Mock 预览")}</Button></>}</Flex>
      {conflict && <Callout.Root color="amber" role="alert"><Callout.Text>{t("共享工作区已被其他成员更新。选择要保留的版本。")}</Callout.Text><Flex gap="2" wrap="wrap" mt="2"><Button variant="soft" onClick={() => { setDraft(conflict); setSaved(conflict); setConflict(null); setError(""); }}>{t("使用服务器版本")}</Button><Button variant="soft" onClick={() => { setSaved(conflict); setDraft((currentDraft) => currentDraft ? { ...currentDraft, revision: conflict.revision } : null); setConflict(null); setError(""); }}>{t("保留我的修改")}</Button></Flex></Callout.Root>}
      {canEdit && saved && (tab === "resources" || tab === "scenarios") && <SharedRunOptions key={scope} value={runConfig} datasets={saved.data.datasets ?? []} scope={`${scope}/${saved.revision}`} workspaceId={workspaceId} disabled={running || !!dataBusy} dark={dark} onChange={(value) => { if (!running && !dataBusy && current(scope)) { invalidateDraftResults(); setRunConfigState({ scope, value }); } }} onBusyChange={(busy) => { if (current(scope)) setRunDataBusy({ scope, busy }); }} previewSource={previewRunData} />}
      {tab === "resources" && <>
        {canEdit ? <Field label={t("工作区名称")}><TextField.Root value={draft.name} onChange={(event) => { invalidateDraftResults(); setDraft((currentDraft) => currentDraft ? { ...currentDraft, name: event.target.value } : null); }} /></Field> : <Text size="2">{draft.name}</Text>}
        {canEdit && <Button variant="outline" onClick={() => setCollectionSettings({ kind: "create", parent_id: null })}>{t("新建集合")}</Button>}
        {draft.data.collections.length === 0 ? <Text size="2" color="gray">{t("此工作区还没有集合")}</Text> : <>
          <Choice label={t("集合")} value={collection?.id ?? ""} onChange={(value) => { setCollectionId(value); setRequestId(""); }} options={draft.data.collections.map((item) => ({ value: item.id, label: item.name }))} />
          {collection && <Flex align="center" justify="between" gap="2" wrap="wrap"><Heading size="3">{collection.name}</Heading>{canEdit && <Flex gap="2" wrap="wrap"><Button variant="outline" onClick={() => setCollectionSettings({ kind: "edit", id: collection.id })}>{t("集合与目录设置")}</Button><Button variant="outline" onClick={() => setCollectionSettings({ kind: "create", parent_id: collection.id })}>{t("新建目录")}</Button><Button variant="outline" onClick={() => addHttpRequest()}>{t("新增 HTTP 请求")}</Button><Button variant="outline" onClick={() => addHttpRequest("data")}>{t("新增 Data 请求")}</Button><Button variant="outline" onClick={() => addHttpRequest("graphql")}>{t("新增 GraphQL 请求")}</Button><Button variant="outline" onClick={() => addHttpRequest("sse")}>{t("新增 SSE 请求")}</Button><Button variant="outline" onClick={() => addHttpRequest("websocket")}>{t("新增 WebSocket 请求")}</Button><Button loading={running} disabled={running || dirty || !runOptionsValid || collection.requests.length === 0} onClick={() => void runCollection()}>{t("运行集合")}</Button></Flex>}</Flex>}
          {canEdit && collection && <SharedResourceActions key={`${scope}/${saved?.revision}/${conflict?.revision ?? ""}`} data={draft.data} workspaceName={draft.name} collectionId={collection.id} requestId={request?.id} onChange={applyResourceChange} />}
          {canEdit && dirty && <Text size="1" color="gray">{t("先保存修改，再运行集合。")}</Text>}
          {runResult && <Text role="status" size="2">{t("通过")}: {runResult.passed} · {t("失败")}: {runResult.failed}{runResult.cancelled && <> · {t("已取消")}</>}</Text>}
          {collection && collection.requests.length > 0 && <>
            <Choice label={t("请求")} value={request?.id ?? ""} onChange={setRequestId} options={collection.requests.map((item) => ({ value: item.id, label: `${item.method} ${item.name}` }))} />
            {request && (canEdit ? <Flex direction="column" gap="3">
              <Field label={t("请求名称")}><TextField.Root value={request.name} onChange={(event) => updateSelectedRequest({ name: event.target.value })} /></Field>
              {request.protocol?.kind !== "data" && <Flex className="shared-request-line" gap="2" wrap="wrap">
                {!graphqlRequest && !httpLiveRequest && <Choice label={t("方法")} value={request.method} onChange={(method) => updateSelectedRequest({ method })} options={["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"].map((value) => ({ value, label: value }))} />}
                <Field label={t("请求地址")}><TextField.Root value={request.url} onChange={(event) => updateSelectedRequest({ url: event.target.value })} /></Field>
              </Flex>}
              <Field label={t("描述")}><TextArea value={request.description} onChange={(event) => updateSelectedRequest({ description: event.target.value })} /></Field>
              {httpRequest && <SharedRequestSpecification key={`${scope}/${collection.id}/${request.id}/${saved?.revision}`} request={request} specifications={draft.data.specifications ?? []} onChange={updateSelectedRequest} />}
              {request.protocol?.kind === "data" && <SharedDataRequestEditor key={`${scope}/${collection.id}/${request.id}/${saved?.revision}/${conflict?.revision ?? ""}`} request={request} dark={dark} isCurrent={() => datasetContextCurrent() && !conflict} onChange={updateSelectedRequest} />}
              {(httpRequest || graphqlRequest || httpLiveRequest) && <Tabs.Root key={`${collection.id}/${request.id}/${saved?.revision}`} value={((graphqlRequest || httpLiveRequest) && requestEditorTab === "body") || (httpLiveRequest && requestEditorTab === "assertions") ? "headers" : requestEditorTab} onValueChange={setRequestEditorTab}>
                <Tabs.List className="shared-request-tabs">
                  {[
                    { value: "query", label: t("参数") }, { value: "headers", label: t("请求头") },
                    { value: "body", label: t("请求体") }, { value: "auth", label: t("鉴权") },
                    { value: "assertions", label: t("断言") }, { value: "scripts", label: t("脚本") },
                    { value: "network", label: t("网络设置") },
                  ].filter((tab) => (!graphqlRequest || tab.value !== "body") && (!httpLiveRequest || !["body", "assertions"].includes(tab.value))).map(({ value, label }) => <Tabs.Trigger key={value} value={value}>{label}</Tabs.Trigger>)}
                </Tabs.List>
                <Tabs.Content value="query"><PairEditor rows={request.query} onChange={(query) => updateSelectedRequest({ query })} keyLabel={t("参数名")} valueLabel={t("参数值")} /></Tabs.Content>
                <Tabs.Content value="headers"><PairEditor rows={request.headers} onChange={(headers) => updateSelectedRequest({ headers })} keyLabel="Header" valueLabel={t("值")} secrets /></Tabs.Content>
                <Tabs.Content value="body">
                  <Flex direction="column" gap="2">
                    <Choice label={t("请求体格式")} value={request.body_kind} onChange={(kind) => updateSelectedRequest(bodyModePatch(request, kind as RequestSpec["body_kind"]))} options={[{ value: "none", label: t("无请求体") }, { value: "json", label: "JSON" }, { value: "text", label: "Raw Text" }, { value: "form", label: "x-www-form-urlencoded" }, { value: "binary", label: t("二进制文件") }, { value: "multipart", label: "multipart/form-data" }]} />
                    {["binary", "multipart"].includes(request.body_kind) ? <RequestBodyEditor key={`${scope}/${collection.id}/${request.id}/${saved?.revision}/${request.body_kind}`} kind={request.body_kind} value={request.body} change={(body) => updateSelectedRequest({ body })} busy={saving || sending || running} dark={dark} /> : request.body_kind === "none" ? <Text size="2" color="gray">{t("此请求不发送 Body。")}</Text> : <Editor label={t("请求正文")} value={request.body} onChange={(body) => updateSelectedRequest({ body })} dark={dark} jsonMode={request.body_kind === "json"} height="210px" />}
                  </Flex>
                </Tabs.Content>
                <Tabs.Content value="auth"><RequestAuthEditor auth={request.auth} change={(auth) => updateSelectedRequest({ auth })} dark={dark} collectionId={collection?.id ?? null} tokenManagement={false} {...authenticationEligibility(request)} /></Tabs.Content>
                <Tabs.Content value="assertions"><AssertionsEditor request={request} update={updateSelectedRequest} dark={dark} /><ExtractionsEditor request={request} update={updateSelectedRequest} /></Tabs.Content>
                <Tabs.Content value="scripts"><RequestScripts request={request} update={updateSelectedRequest} dark={dark} /></Tabs.Content>
                <Tabs.Content value="network"><Flex direction="column" gap="3"><NetworkSettings key={`${scope}/${collection.id}/${request.id}/${saved?.revision}/${conflict?.revision ?? ""}`} value={request.network} change={(network) => { if (datasetContextCurrent() && selectedRequestRef.current === request.id && !conflict) updateSelectedRequest({ network }); }} disabled={saving || !!conflict} ntlm={request.auth.kind === "ntlm"} websocket={websocketRequest} />
                  <Field label={t("请求超时 (ms)")}><TextField.Root type="number" min="100" max="120000" value={request.timeout_ms} onChange={(event) => updateSelectedRequest({ timeout_ms: Number(event.target.value) })} /></Field>
                  <label className="checkbox-label"><Checkbox checked={request.verify_tls} onCheckedChange={(value) => updateSelectedRequest({ verify_tls: value === true })} />{t("验证 TLS 证书")}</label>
                  <label className="checkbox-label"><Checkbox checked={request.follow_redirects} onCheckedChange={(value) => updateSelectedRequest({ follow_redirects: value === true })} />{t("自动跟随重定向")}</label>
                </Flex></Tabs.Content>
              </Tabs.Root>}
            </Flex> : <Flex direction="column" gap="1"><Text weight="medium">{request.method} {request.name}</Text><Text size="2" style={{ overflowWrap: "anywhere" }}>{request.url}</Text><Text size="2" color="gray">{request.description}</Text></Flex>)}
          </>}
          {canEdit && request && graphqlRequest && saved && collection && <SharedGraphQLWorkbench key={`${scope}/${request.id}/${saved.revision}/${conflict?.revision ?? ""}`} scope={`${scope}/${request.id}/${saved.revision}/${conflict?.revision ?? ""}`} base={base} request={request} saved={saved} data={draft.data} collectionId={collection.id} dark={dark} dirty={dirty} saving={saving} conflict={!!conflict} isCurrent={() => datasetContextCurrent() && selectedRequestRef.current === request.id} onChange={updateSelectedRequest} onAttach={attachGraphQLSchema} />}
          {canEdit && request && liveRequest && saved && <SharedLiveSessionPanel key={`${scope}/${request.id}/${saved.revision}/${dirty}`} base={base} request={request} revision={saved.revision} environmentId={saved.data.active_environment_id} dirty={dirty} saving={saving} conflict={!!conflict} dark={dark} isCurrent={() => datasetContextCurrent() && selectedRequestRef.current === request.id && !conflict} onSaveResultDataset={saveResultDataset} />}
          {canEdit && request && !liveRequest && !graphqlRequest && <Flex direction="column" gap="2">
            {dirty && httpRequest ? <><Button loading={sending} disabled={sending || saving || !!conflict} onClick={() => void sendDraftRequest()}>{t("发送草稿请求")}</Button><Text size="1" color="gray">{t("草稿请求仅供本次执行，保存后才会更新工作区。")}</Text></> : <Button loading={sending} disabled={sending || dirty} onClick={() => void sendSavedRequest()}>{t("发送已保存请求")}</Button>}
            {requestResponse && <><Text role="status" size="2">{responseSource === "draft" ? t("草稿请求执行结果") : t("已保存请求执行结果")}: {requestResponse.status} {requestResponse.status_text} · {requestResponse.elapsed_ms} ms</Text><TextArea aria-label={t("响应正文预览")} readOnly value={requestResponse.body.slice(0, 16 * 1024)} />{requestResponse.body.length > 16 * 1024 && <Text size="1" color="gray">{t("仅显示响应正文前 16 KiB。")}</Text>}</>}
          </Flex>}
        </>}
        {canEdit && <Button loading={saving} disabled={!dirty || saving || !!conflict || invalidDocuments} onClick={() => void save()}>{t("保存共享工作区")}</Button>}
      </>}
      {tab === "specifications" && saved && <SharedSpecificationsPanel key={`${scope}/${saved.revision}/${conflict?.revision ?? ""}`} scope={`${scope}/${saved.revision}/${conflict?.revision ?? ""}`} data={draft.data} base={base} revision={saved.revision} dark={dark} canEdit={canEdit} dirty={dirty} saving={saving} canSave={!saving && !conflict && !invalidDocuments} isCurrent={() => datasetContextCurrent() && !conflict} onChange={updateSpecificationData} onSave={() => void save()} />}
      {tab === "datasets" && canEdit && saved && <SharedDatasetsPanel key={`${scope}/${saved.revision}/${conflict?.revision ?? ""}`} scope={`${scope}/${saved.revision}/${conflict?.revision ?? ""}`} data={draft.data} base={base} revision={saved.revision} dark={dark} dirty={dirty} saving={saving} canSave={!saving && !conflict && !invalidDocuments} isCurrent={datasetContextCurrent} onChange={updateDatasets} onSave={() => void save()} />}
      {tab === "scenarios" && canEdit && <SharedScenariosPanel key={`${scope}/${collection?.id ?? ""}/${saved?.revision}/${conflict?.revision ?? ""}`} data={draft.data} collectionId={collection?.id ?? ""} dirty={dirty} saving={saving} canSave={!saving && !conflict && !invalidDocuments} running={running} runOptionsValid={runOptionsValid} result={runResult} onSelectCollection={(value) => { setCollectionId(value); setRequestId(""); }} onChange={updateScenarios} onSave={() => void save()} onRun={(id) => void runCollection(id)} />}
      {tab === "documents" && <SharedDocumentsPanel key={`${scope}/${saved?.revision}`} data={draft.data} canEdit={canEdit} dark={dark} dirty={dirty} saving={saving} canSave={!saving && !conflict && !invalidDocuments} conflictRevision={conflict?.revision ?? null} onChange={updateDocuments} onSave={() => void save()} onOpenRequest={openSharedRequest} />}
      {tab === "variables" && <SharedVariablesPanel key={`${scope}/${saved?.revision}`} data={draft.data} canEdit={canEdit} dirty={dirty} saving={saving} canSave={!saving && !conflict && !invalidDocuments} conflictRevision={conflict?.revision ?? null} onChange={updateVariables} onSave={() => void save()} />}
      {tab === "history" && canEdit && <Flex direction="column" gap="2">{history.isPending && <Text role="status">{t("正在加载执行历史…")}</Text>}{history.error && <Callout.Root color="red" role="alert"><Callout.Text>{liveError(history.error)}</Callout.Text></Callout.Root>}{!history.error && history.data?.length === 0 && <Text size="2" color="gray">{t("暂无执行历史")}</Text>}{!history.error && history.data?.map((item) => <Flex key={item.id} justify="between" gap="2" wrap="wrap"><Text size="2">{item.method} {item.request_name}</Text><Text size="2">{item.status} · {item.created_at}</Text></Flex>)}</Flex>}
      {tab === "reports" && canEdit && <Flex direction="column" gap="2">
        {reports.isPending && <Text role="status">{t("正在加载测试报告…")}</Text>}
        {reports.error && <Callout.Root color="red" role="alert"><Callout.Text>{liveError(reports.error)}</Callout.Text></Callout.Root>}
        {!reports.error && reports.data?.items.length === 0 && <Text size="2" color="gray">{t("暂无测试报告")}</Text>}
        {!reports.error && !!reports.data?.items.length && <Choice label={t("报告导出格式")} value={reportFormat} disabled={!!exportingReport} onChange={setReportFormat} options={[{ value: "html", label: "HTML" }, { value: "json", label: "JSON" }, { value: "csv", label: "CSV" }, { value: "junit", label: "JUnit XML" }]} />}
        {!reports.error && reports.data?.items.map((item) => <Flex key={item.id} justify="between" align="center" gap="2" wrap="wrap"><Text size="2">{item.collection_name}</Text><Text size="2">{t("通过")}: {item.summary.passed} · {t("失败")}: {item.summary.failed}</Text><Button variant="outline" color="gray" disabled={!!exportingReport} loading={exportingReport === item.id} onClick={() => void downloadReport(item.id)}>{t("下载脱敏报告")}</Button></Flex>)}
      </Flex>}
      {tab === "mock" && canEdit && <Flex direction="column" gap="3">
        <Heading size="3">{t("Mock 示例预览")}</Heading>
        {draft.data.collections.length > 0 && <Choice label={t("集合")} value={collection?.id ?? ""} onChange={(value) => { setCollectionId(value); setRequestId(""); setExampleId(""); }} options={draft.data.collections.map((item) => ({ value: item.id, label: item.name }))} />}
        {collection && collection.requests.length > 0 && <Choice label={t("请求")} value={request?.id ?? ""} onChange={(value) => { setRequestId(value); setExampleId(""); }} options={collection.requests.map((item) => ({ value: item.id, label: `${item.method} ${item.name}` }))} />}
        {request && <Button variant="outline" onClick={addExample}>{t("新增响应示例")}</Button>}
        {request && request.examples.length > 0 && <Choice label={t("响应示例")} value={example?.id ?? ""} onChange={setExampleId} options={request.examples.map((item) => ({ value: item.id, label: `${item.status} ${item.name}` }))} />}
        {!example && <Text size="2" color="gray">{t("所选请求没有响应示例")}</Text>}
        {example && <Flex direction="column" gap="2">
          <Field label={t("示例名称")}><TextField.Root value={example.name} onChange={(event) => updateExample((item) => ({ ...item, name: event.target.value }))} /></Field>
          <Flex className="shared-request-line" gap="2" wrap="wrap">
            <Field label={t("状态码")}><TextField.Root inputMode="numeric" maxLength={3} value={exampleStatusInput} onChange={(event) => { const value = event.target.value; invalidateDraftResults(); setExampleStatusInput(value); const status = Number(value); if (/^[2-5]\d\d$/.test(value) && status <= 599) updateExample((item) => ({ ...item, status })); }} onBlur={() => { if (statusPending) setExampleStatusInput(String(example.status)); }} /></Field>
          </Flex>
          <MockTrafficControls example={example} onChange={(patch) => updateExample((item) => ({ ...item, ...patch }))} />
          <SharedMockSchemaControls key={`${scope}/${request?.id}/${example.id}/${saved?.revision}/${conflict?.revision ?? ""}`} example={example} request={request!} specifications={draft.data.specifications ?? []} onSelectLinked={(pointer) => void selectLinkedSchema(pointer)} dark={dark} datasets={draft.data.datasets ?? []} dataEnabled={httpRequest && draft.data.script_mode === "v2"} onChange={(patch) => updateExample((item) => ({ ...item, ...patch }))} />
          {statusPending && <Text role="alert" size="1" color="red">{t("状态码必须为 200 到 599。")}</Text>}
          {linkingSchema && <Text size="1" color="gray">{t("正在校验关联响应…")}</Text>}
          <Field label={t("响应正文")}><TextArea value={example.body} onChange={(event) => updateExample((item) => ({ ...item, body: event.target.value }))} /></Field>
          <div><Text as="p" size="2" weight="medium">{t("响应头")}</Text><PairEditor rows={example.headers} onChange={(headers) => updateExample((item) => ({ ...item, headers }))} keyLabel="Header" valueLabel={t("值")} secrets /></div>
          {request && matchRoutePath ? <Text as="p" size="1" color="gray" style={{ overflowWrap: "anywhere" }}>{t("匹配路由")}: <code>{`${request.method} ${base}/mocks${matchRoutePath}`}</code></Text> : <Text size="1" color="gray">{t("Mock 匹配路由需要字面量 HTTP URL；路径变量使用单层 {name}。")}</Text>}
          {example.mock_data && <><MockDataEditor binding={example.mock_data} datasets={draft.data.datasets ?? []} onChange={(mock_data) => updateExample((item) => ({ ...item, mock_data }))} /><Text size="1" color="gray">{t("数据绑定启用时，保存的静态正文不会作为失败回退。")}</Text></>}
          <MockMatchingEditor rule={example.mock_match} onChange={(mock_match) => updateExample((item) => ({ ...item, mock_match }))} />
          <MockScriptEditor script={example.mock_script ?? ""} enabled={draft.data.script_mode === "v2"} dark={dark} onChange={(mock_script) => updateExample((item) => ({ ...item, mock_script }))} />
          <Button variant="outline" color="red" onClick={() => { updateRequest((item) => ({ ...item, examples: item.examples.filter((entry) => entry.id !== example.id) })); setExampleId(""); }}>{t("删除示例")}</Button>
        </Flex>}
        {example && <Button loading={mocking} disabled={mocking || dirty || statusPending || !!conflict || !!example.mock_data?.parameters?.length} onClick={() => void previewExample()}>{t("预览已保存 Mock 示例")}</Button>}
        {!!example?.mock_data?.parameters?.length && <Text size="1" color="gray">{t("带请求参数的数据绑定请通过匹配路由调用。")}</Text>}
        {dirty && <Text size="1" color="gray">{t("先保存修改，再预览 Mock 示例。")}</Text>}
        {mockPreview && <><Text role="status" size="2">{t("Mock 响应状态")}: {mockPreview.status}</Text><TextArea aria-label={t("Mock 响应正文")} readOnly value={mockPreview.body.slice(0, 16 * 1024)} />{mockPreview.body.length > 16 * 1024 && <Text size="1" color="gray">{t("仅显示响应正文前 16 KiB。")}</Text>}</>}
        <Button loading={saving} disabled={!dirty || saving || statusPending || !!conflict || invalidDocuments} onClick={() => void save()}>{t("保存共享工作区")}</Button>
        <Heading size="3">{t("Mock 调用记录")}</Heading>
        {mockCalls.isPending && <Text role="status">{t("正在加载 Mock 调用记录…")}</Text>}
        {mockCalls.error && <Callout.Root color="red" role="alert"><Callout.Text>{liveError(mockCalls.error)}</Callout.Text></Callout.Root>}
        {!mockCalls.error && mockCalls.data?.length === 0 && <Text size="2" color="gray">{t("暂无 Mock 调用记录")}</Text>}
        {!mockCalls.error && mockCalls.data?.map((item) => <Flex key={item.id} direction="column" gap="1"><Text size="2">{t("请求")}: {item.request_id ?? "—"} · {t("响应示例")}: {item.example_id ?? "—"}</Text><Flex justify="between" gap="2" wrap="wrap"><Text size="2">{item.response_status} · {item.outcome === "served" ? t("已响应") : item.outcome === "unmatched" ? t("未匹配") : item.outcome === "simulated_failure" ? t("模拟故障") : item.outcome === "rate_limited" ? t("已限流") : t("失败")}</Text><Text size="2">{new Date(item.at).toLocaleString(language)}</Text></Flex></Flex>)}
      </Flex>}
    </Flex>}
    {collectionSettings && canEdit && draft && loadedScope === scope && <SharedCollectionSettingsDialog target={collectionSettings} data={draft.data} dark={dark} onApply={applyCollectionSettings} close={() => setCollectionSettings(null)} />}
  </main>;
}
