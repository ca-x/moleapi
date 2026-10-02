#!/usr/bin/env python3
"""Capture official documentation using agent-browser; retain provenance, never infer support from a slug."""
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from urllib.parse import urlparse
import argparse, datetime, hashlib, json, subprocess, threading
from markdown_headings import extract_headings

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / 'docs/references'


def capture(row):
    parsed = urlparse(row['url'])
    if parsed.hostname not in {'docs.apifox.com', 'learning.postman.com'}:
        raise ValueError('Only official documentation domains are allowed')
    key = hashlib.sha256(row['url'].encode()).hexdigest()[:20]
    path = REF / 'raw' / (row['vendor'] + '-' + key + '.json')
    if path.exists():
        saved = json.loads(path.read_text())
        if saved.get('success') and saved.get('data', {}).get('status') == 200:
            return metadata(row, saved, path)
    result = None
    error = ''
    for attempt in range(2):
        try:
            proc = subprocess.run(['agent-browser', '--session', 'moleapi-research-' + str(threading.get_ident()), 'read', row['url'], '--json'], capture_output=True, text=True, timeout=35)
            result = json.loads(proc.stdout)
            if result.get('success'):
                break
            error = str(result.get('error', proc.stderr))[:400]
        except (subprocess.TimeoutExpired, json.JSONDecodeError) as exc:
            error = str(exc)[:400]
    if result is None:
        result = {'success': False, 'error': error}
    result['_captured_at'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    path.write_text(json.dumps(result, ensure_ascii=False, indent=2))
    return metadata(row, result, path)


def metadata(row, result, path):
    data = result.get('data') or {}
    content = data.get('content', '')
    headings = extract_headings(content)
    final_url = data.get('finalUrl', data.get('url', ''))
    expected_path = urlparse(row['url']).path.rstrip('/').removesuffix('.md')
    actual_path = urlparse(final_url).path.rstrip('/').removesuffix('.md')
    verified = bool(result.get('success') and data.get('status') == 200 and content and expected_path == actual_path and not data.get('truncated'))
    availability_markers = ('This feature is available', 'available on Postman', 'requires a Postman', 'available on Enterprise', 'only supported', 'only available', 'desktop app only', 'currently in beta', 'aren’t available', "aren't available", 'not available in', 'for Enterprise customers', '商业旗舰版', 'Beta 阶段', '仅支持', '不支持', '仅限')
    availability = [line.strip()[:800] for line in content.splitlines() if any(marker.casefold() in line.casefold() for marker in availability_markers)]
    return {**row, 'availability_evidence': availability if verified else [], 'verified': verified, 'retrieved_at': result.get('_captured_at') or datetime.datetime.fromtimestamp(path.stat().st_mtime, datetime.timezone.utc).isoformat(), 'title_verified': headings[0]['title'] if verified and headings else None, 'headings': headings if verified else [], 'characters': len(content), 'source': data.get('source'), 'status': data.get('status'), 'final_url': final_url, 'raw_file': str(path.relative_to(ROOT)), 'sha256': hashlib.sha256(content.encode()).hexdigest(), 'error': result.get('error') if not verified else None}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--workers', type=int, default=8)
    parser.add_argument('--all', action='store_true', help='Retrieve all current product documentation; large batch')
    args = parser.parse_args()
    fox = json.loads((REF / 'apifox-catalog.json').read_text())
    postman = json.loads((REF / 'postman-catalog.json').read_text())
    fox = [{**x, 'vendor': 'apifox'} for x in fox if '?nav=' not in x['url'] and not x['category'].startswith(('常见问题', '身份验证 &', '服务与', '参考资料', '最佳实践')) and x['category'].split(' > ')[0] not in {'导入/导出','发送请求','调试 API','Markdown 相关','收费与部署','Web 端与客户端','MCP Server','公告','AI 功能','WebService','入门'}]
    focus = {'use','tests-and-scripts','monitoring-your-api','design-apis','collaborating-in-postman','publishing-your-api','api-catalog','insights','reports','sdk-generator','cli-generator','postman-api-network','administration','api-governance','postman-cli','integrations','flows','getting-started','reference','fern','observe','publish','billing'}
    postman = [{**x, 'vendor': 'postman'} for x in postman if x['category'] in focus and '/v11/' not in x['url']]
    important = {'authorization-types','parameters','headers','test-data','request-settings','request-basics','postman-basics','navigating-postman','postman-elements','about-ai','overview','protocols','responses','examples','cookies','visualizer','variables-intro','environment-variables','team-environments','native-git','postman-vault-secrets','postman-vault-integrations','intro-to-scripts','postman-cli-overview','intro-monitors','performance-testing','local-mock-servers','set-up-mock-servers','simulate-conditions','sdk-generator','cli-generator','postman-sandbox','roles-and-permissions','intro-sso','scim-provisioning-overview','byok-encryption','audit-logs','api-governance-overview','install-app','api-documentation-overview','create-datasets','tests-and-scripts','spec-hub-visual-editor','collections-schemas','oauth-debugger','using-service-definition','mqtt-request-interface','graphql-client-interface','grpc-request-interface','postman-api','intro-to-collection-runs','run-collection-with-mock'}
    if not args.all:
        postman = [x for x in postman if urlparse(x['url']).path.rstrip('/').split('/')[-1] in important]
    selected = fox + postman
    (REF / 'fetch-plan.json').write_text(json.dumps(selected, ensure_ascii=False, indent=2))
    print(f'Capturing {len(fox)} Apifox and {len(postman)} Postman documents via agent-browser', flush=True)
    results = []
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = [pool.submit(capture, row) for row in selected]
        for i, future in enumerate(as_completed(futures), 1):
            results.append(future.result())
            if i % 20 == 0:
                print(f'{i}/{len(selected)} complete; {sum(x["verified"] for x in results)} verified', flush=True)
    results.sort(key=lambda x: (x['vendor'], x['url']))
    (REF / 'fetch-manifest.json').write_text(json.dumps(results, ensure_ascii=False, indent=2))
    print(f'Finished: {sum(x["verified"] for x in results)} verified; {sum(not x["verified"] for x in results)} failed/redirected. All failures retained.', flush=True)

if __name__ == '__main__':
    main()
