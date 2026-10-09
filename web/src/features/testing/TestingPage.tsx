import {t,useLanguage} from "../../shared/i18n";
import {useState} from "react";
import {Badge,Button,Card,Flex,Heading,Text,Callout} from "@radix-ui/themes";
import {FlaskConical} from "lucide-react";
import {Choice} from "../../shared/ui";
import {useWorkbench} from "../workbench/context";
import RunnerDataset,{runnerOptions,type RunnerConfig} from "./RunnerDataset";
import SavedDatasets from "./SavedDatasets";
import SavedScenarios from "./SavedScenarios";
export default function TestingPage(){
 useLanguage();const state=useWorkbench();const {draft,runCollection,setRunCollection,runResult,runnerBusy,run,stopRunner}=state;
 const scope=JSON.stringify([state.accountId,draft?.id]);
 const [source,setSource]=useState<{scope:string;value:RunnerConfig}|null>(null),[invalid,setInvalid]=useState(false);
 const [previewBusy,setPreviewBusy]=useState<{scope:string;busy:boolean}|null>(null);
 const dataBusy=previewBusy?.scope===scope&&previewBusy.busy;
 const config=source?.scope===scope?source.value:{format:"json" as const,source:"",iterations:""};
 const [savedSelection,setSavedSelection]=useState<{scope:string;id:string}|null>(null);
 const savedId=savedSelection?.scope===scope&&draft?.data.datasets?.some(dataset=>dataset.id===savedSelection.id)?savedSelection.id:"";
 const saved=draft?.data.datasets?.find(dataset=>dataset.id===savedId);
 const collectionId=runCollection||draft?.data.collections[0]?.id||"";
 const [scenarioSelection,setScenarioSelection]=useState<{scope:string;id:string}|null>(null);
 const scenarios=(draft?.data.scenarios??[]).filter(scenario=>scenario.collection_id===collectionId);
 const scenarioId=scenarioSelection?.scope===scope&&scenarios.some(scenario=>scenario.id===scenarioSelection.id)?scenarioSelection.id:"";
 function start(){try{const options=runnerOptions(savedId?{...config,source:""}:config);if(savedId){if(!saved?.source)throw new Error("dataset source missing");options.dataset_id=savedId;}if(scenarioId)options.scenario_id=scenarioId;setInvalid(false);void run(options);}catch{setInvalid(true);}}
 if(!draft)return null;
 return <div className="page-panel"><div className="page-heading"><div><Heading size="5">{t("集合测试")}</Heading><Text size="2" color="gray">{t("按顺序运行集合内的请求，并检查断言。")}</Text></div></div>
 <Flex gap="3" align="center" wrap="wrap"><Choice value={runCollection||draft.data.collections[0]?.id||"none"} onChange={setRunCollection} options={draft.data.collections.map(collection=>({value:collection.id,label:collection.name}))} label={t("测试集合")} disabled={runnerBusy}/><Button loading={runnerBusy} disabled={!draft.data.collections.length||dataBusy} onClick={start}><FlaskConical size={16}/>{t("运行集合")}</Button>{runnerBusy&&<Button variant="soft" onClick={()=>void stopRunner()}>{t("停止集合运行")}</Button>}</Flex>
 <Choice label={t("运行场景")} value={scenarioId||"collection"} disabled={runnerBusy} options={[{value:"collection",label:t("集合原始顺序")},...scenarios.map(scenario=>({value:scenario.id,label:scenario.name}))]} onChange={id=>setScenarioSelection({scope,id:id==="collection"?"":id})}/>
 <SavedScenarios key={scope+collectionId} collectionId={collectionId} disabled={runnerBusy}/>
 <Choice label={t("运行数据集")} value={savedId||"temporary"} disabled={runnerBusy} options={[{value:"temporary",label:t("临时数据 / 无数据")},...(draft.data.datasets??[]).map(dataset=>({value:dataset.id,label:dataset.name}))]} onChange={id=>{setSavedSelection({scope,id:id==="temporary"?"":id});setInvalid(false);}}/>
 <RunnerDataset key={scope+savedId} workspaceId={draft.id} scope={scope} value={savedId?{...config,format:saved?.source?.format??"json",source:saved?.source?.source??""}:config} onBusyChange={busy=>setPreviewBusy({scope,busy})} disabled={runnerBusy} sourceReadOnly={!!savedId} dark={state.dark} onChange={value=>{setSource({scope,value:savedId?{...config,iterations:value.iterations}:value});setInvalid(false);}}/>
 {savedId&&!saved?.source&&<Text>{t("数据源缺失，请重新导入。")}</Text>}
 <SavedDatasets key={scope+draft.revision} scope={scope+draft.revision} disabled={runnerBusy}/>
 {invalid&&<Callout.Root color="red"><Callout.Text role="alert">{t("测试数据无效，请检查格式、迭代次数和大小限制。")}</Callout.Text></Callout.Root>}
 {runResult&&<><Flex gap="4" align="center"><Badge color="green">{t("通过")} {runResult.passed}</Badge><Badge color="red">{t("失败")} {runResult.failed}</Badge><Text size="2" color="gray">{runResult.elapsed_ms} ms</Text>{!!runResult.skipped&&<Badge color="gray">{t("跳过")} {runResult.skipped}</Badge>}</Flex>
 {runResult.stopped_reason&&<Text role="status">{runResult.stopped_reason==="parallel_variable_conflict"?t("并行变量写入冲突，本块更新未应用。"):runResult.stopped_reason==="parallel_control"||runResult.stopped_reason==="parallel_entry"?t("并行块流程跳转无效，场景已停止。"):runResult.stopped_reason==="condition_error"?t("场景条件执行失败，请检查布尔结果和执行限制。"):runResult.stopped_reason==="deadline"?t("集合运行达到时间限制。"):runResult.stopped_reason==="variable_limit"?t("集合运行达到变量大小限制。"):runResult.stopped_reason==="script_stop"?t("脚本已结束集合流程。"):runResult.stopped_reason==="step_limit"?t("集合流程达到最大步骤限制。"):runResult.stopped_reason.startsWith("next_request_")?t("下一请求不存在或名称不唯一，流程已停止。"):t("集合运行已取消。")}</Text>}
 {runResult.iterations?.map(iteration=><Text key={iteration.iteration}>{t("第 {{iteration}} 轮：通过 {{passed}}，失败 {{failed}}。",{iteration:iteration.iteration+1,passed:iteration.passed,failed:iteration.failed})}{iteration.script_stopped&&` · ${t("脚本已结束本轮流程。")}`}{iteration.scenario_stopped&&` · ${t("场景分支已结束本轮。")}`}</Text>)}
 {runResult.results.map((item,index)=><Card key={`${item.iteration??0}:${item.collection_id??""}:${item.request_id}:${index}`}><Flex justify="between"><Text weight="medium">{item.step_name||item.request_name}</Text>{item.parallel_name&&<Text size="1" color="gray">{t("并行块")} · {item.parallel_name}</Text>}{item.step_group&&<Text size="1" color="gray">{item.step_group}</Text>}{item.iteration!==undefined&&<Text size="1">{t("第 {{iteration}} 轮",{iteration:item.iteration+1})}</Text>}{item.condition_skipped?<Badge color="gray">{t("请求因场景条件不成立而跳过。")}</Badge>:item.response?.skipped?<Badge color="gray">{t("请求已由脚本跳过。")}</Badge>:(item.response||item.status)&&<Badge color={item.response?item.response.tests.every(test=>test.passed)?"green":"red":"gray"}>{item.response?.status??item.status}</Badge>}</Flex>{item.error&&<Text color="red" size="2">{item.error==="Scenario condition failed; check its boolean result and execution limits"?t("场景条件执行失败，请检查布尔结果和执行限制。"):item.error}</Text>}{item.response_omitted&&<Text size="1">{t("响应详情因报告大小限制被省略。")}</Text>}{item.response?.tests.map(test=><Text as="p" size="2" key={test.id} color={test.passed?"green":"red"}>{test.passed?t("通过"):t("失败")} · {test.name} · {test.actual}</Text>)}</Card>)}</>}
 </div>;
}
