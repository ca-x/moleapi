#!/usr/bin/env python3
"""Concrete public UI/config choices, separate from domain-wide parity targets."""
from pathlib import Path
import json
ROOT=Path(__file__).resolve().parents[1]
REF=ROOT/'docs/references'
manifest=json.loads((REF/'fetch-manifest.json').read_text())
lookup={r['url'].rstrip('/'):r for r in manifest}
controls=[]


def add(vendor,path,name,options,entry_path='',restrictions=(),source_heading=None):
    url=('https://docs.apifox.com/'+path+'.md') if vendor=='apifox' else 'https://learning.postman.com/docs/'+path
    row=lookup.get(url)
    if not row or not row['verified']:raise ValueError('Concrete options require read evidence: '+url)
    line=next((h['source_line'] for h in row['headings'] if h['title']==source_heading),None) if source_heading else None
    controls.append({'id':f'CTRL-{len(controls)+1:03d}','vendor':vendor,'control':name,'options':options,'ui_entry_path':entry_path or None,'ui_entry_path_evidence':'documented' if entry_path else 'not_observed','source_url':url,'source_heading':source_heading,'source_line':line,'source_anchor':None,'restrictions':list(restrictions),'implementation_status':'not_implemented'})


add('apifox','request-params-and-body','请求 Body 格式',['form-data','x-www-form-urlencoded','JSON','XML','raw','binary','GraphQL','Msgpack'],source_heading='请求体')
add('apifox','authorization-types','Auth 类型',['从父级继承','No Auth','API Key','Bearer Token','JWT','Basic Auth','Digest Auth','OAuth 1.0','OAuth 2.0','Hawk Authentication','Kerberos','NTLM','Akamai EdgeGrid'],'接口 → Auth → 类型')
add('postman','use/send-requests/authorization/authorization-types','基础 Authorization 类型',['No Auth','API Key','Bearer Token','JWT Bearer','Basic Auth'],'Request → Authorization → Auth Type',restrictions=['更多授权类型见各自官方页面，不以此基本类型列表表示完整下拉菜单。'])
add('apifox','global-environment-session-variables','变量作用域（高到低）',['临时','测试数据','环境','模块','项目全局','团队全局'],'环境管理',restrictions=['临时 > 测试数据 > 环境 > 模块 > 项目全局 > 团队全局','远程值同步；本地值不共享，设置后覆盖远程值。'],source_heading='变量类型与优先级')
add('apifox','vault-secrets','外部 Vault provider',['HashiCorp Vault','Azure Key Vault','AWS Secrets Manager'],'团队资源/项目 → 密钥库提供商',restrictions=['商业旗舰版','值加密保存本地，不共享；名称和元数据可共享。'],source_heading='配置密钥库提供商')
add('apifox','flow-control-conditions','场景控制节点',['分组','ForEach 循环','For 循环','条件分支','等待时间'],'自动化测试 → 场景用例 → 添加步骤')
add('apifox','flow-control-conditions','条件判断运算',['等于','不等于','存在','不存在','小于','小于或等于','大于','大于或等于','正则匹配','包含','不包含','为空','不为空','属于集合','不属于集合'],source_heading='判断规则')
add('apifox','page-layout','工作台模块',['接口管理','自动化测试','分享文档','请求历史','项目设置','邀请成员'],source_heading='侧边栏')
add('postman','getting-started/basics/navigating-postman','侧栏视图',['Items','Services','History','Local Files'],source_heading='Sidebar')
add('postman','getting-started/basics/navigating-postman','默认资源分组',['Collections','Environments','Documents','Specs','SDKs','Datasets','Flows'],'Sidebar → Customize sidebar',source_heading='Change sidebar elements')
add('postman','use/send-requests/protocols/protocols','协议/客户端类型',['HTTP/REST','AI Requests','Data','GraphQL','gRPC','MCP','MQTT','SOAP','UDS/Named Pipes','Webhook','WebSocket/Socket.IO'],restrictions=['不同协议提供独立客户端，不能用统一 HTTP response 字段代替。'])
add('postman','design-apis/specifications/overview','Spec Hub 规范格式',['OpenAPI','AsyncAPI','protobuf 2/3','GraphQL','Smithy 2.0'],source_heading='Supported specification formats')
add('postman','use/native-git/overview','Git/云工作方式',['Local View','Cloud View'],restrictions=['Native Git 仅桌面端','本地 Git 文件与显式 CI/云发布边界需独立处理。'])
add('postman','use/use-collections/collections-schemas','集合格式',['3.0.0 多文件 YAML','2.1.0 JSON 兼容导出'],restrictions=['Newman 不能直接执行 3.0，执行 2.1 导出；3.0 使用 Postman CLI。'])
add('postman','tests-and-scripts/datasets/create-datasets','Dataset 数据源',['Postman Cloud Store','Local File','MySQL','Postgres','SQL Server','JDBC Source','Add with AI'],'Sidebar → + → Dataset',restrictions=['Dataset: Solo/Team/Enterprise','Live database: Team/Enterprise','Custom JDBC: Enterprise','Desktop 全部；Web 仅 Cloud Store/MySQL/Postgres。'],source_heading='Data source types')
add('postman','tests-and-scripts/datasets/create-datasets','数据文件类型',['CSV','JSON','.xlsx','.xls','.ods'],restrictions=['多表格工作表分别成为数据源','Spreadsheet 不提供预览但可以添加。'],source_heading='Data file')
add('postman','tests-and-scripts/datasets/create-datasets','JDBC URL patterns',['MySQL','Postgres','SQL Server','Oracle','Generic'],'Dataset → JDBC Source → Driver → Connection URL',restrictions=['需要 JDBC driver JAR 和可用 Java runtime','JDBC View 使用原数据库 SQL dialect，单数据源，不可跨来源 join。'],source_heading='Supported URL patterns')
add('postman','tests-and-scripts/datasets/create-datasets','数据库/SSH 连接字段',['Host','Port','Database','Username','Password','Table','Schema','SSH host','SSH port','SSH username','SSH private key','SSH host key','Skip SSH host key verification'],'Dataset → Data Source → Connection',restrictions=['Schema 字段适用 Postgres/SQL Server','SSH 的源必须能从 SSH server 网络访问。'])
add('postman','use/send-requests/protocols/data/create-data-request','交互式 Data request source',['Local file','Remote file','MySQL','PostgreSQL'],'Sidebar → + → Data → address bar',restrictions=['桌面或启用 Desktop Agent 的 Web','Local file 仅桌面','Read-only 设置阻止写入/改 schema。'],source_heading='Connect to a data source')
add('postman','administration/enterprise/about-eu-data-residency','账号 Region Preference',['Always Ask for Region Selection','Use EU Region by Default'],'Desktop → Help → Region Preference for New Accounts',restrictions=['Enterprise EU 计划','只改变新账号认证入口，不改变已存在账号','EU 文档列出集成/Partner Workspace/BYOK runs/Live Sessions/API Network/Fern 等能力例外。'],source_heading='Set Postman region preference')
# Preserve actual headings for reporter/CLI options rather than guessing flag values.
for path,name in [('reference/newman-cli/newman-options','Newman CLI 选项分组'),('reference/newman-cli/newman-built-in-reporters','Newman 内置 Reporter'),('postman-cli/postman-cli-options','Postman CLI 选项分组')]:
    row=lookup['https://learning.postman.com/docs/'+path]
    add('postman',path,name,[h['title'] for h in row['headings'] if h['level'] in [2,3]],restrictions=['这里列官方选项章节；具体 flags/默认值查对应正文，不把章节名当成参数值。'])

package={'date':'2026-10-02','scope':'Selected concrete public controls/options; complete public document hierarchy is in feature-options.json. Authenticated and enterprise UI not observed.', 'controls':controls}
(ROOT/'docs/configuration-options.json').write_text(json.dumps(package,ensure_ascii=False,indent=2))
text=['# 具体 UI 与配置选项','','这些记录把已核实的菜单/字段/枚举和已知套餐、平台条件单独保存；它们是完整公开目录的重点提取，不宣称穷尽登录后所有 UI 控件。`source_anchor=null` 表示没有实测远端锚点，原文行号和章节可定位。更多配置项可按 feature-options.json 的公开文档层级继续定位。','']
for c in controls:
    text += [f'## {c["id"]} · {c["vendor"]} · {c["control"]}', '', '**选项：** '+', '.join(c['options']), '', '**入口：** '+(c['ui_entry_path'] or '未现场观察；见官方说明'), '', '**来源：** '+c['source_url']]
    if c['restrictions']:text+=['','**条件：** '+'；'.join(c['restrictions'])]
    text.append('')
(ROOT/'docs/CONFIGURATION-OPTIONS.md').write_text('\n'.join(text))
print(f'{len(controls)} concrete control groups written')
