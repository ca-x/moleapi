#!/usr/bin/env python3
"""Generate browseable official catalogs and traceable feature requirements."""
from pathlib import Path
from urllib.parse import urlparse
import collections, csv, datetime, hashlib, json, re
ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / 'docs/references'
DOCS = ROOT / 'docs'


def canonical(url):
    p = urlparse(url)
    return p.netloc + p.path.rstrip('/').removesuffix('.md')


def main():
    manifest = json.loads((REF / 'fetch-manifest.json').read_text())
    lookup = {canonical(r['url']): r for r in manifest}
    catalogs = {v: json.loads((REF / f'{v}-catalog.json').read_text()) for v in ['apifox', 'postman']}
    pages, options = [], []
    for vendor, catalog in catalogs.items():
        for i, row in enumerate(catalog, 1):
            evidence = lookup.get(canonical(row['url']))
            prefix = 'AF' if vendor == 'apifox' else 'PM'
            page_id = f'{prefix}-{i:04d}'
            record = {**row, 'id': page_id, 'vendor': vendor, 'evidence': 'read' if evidence and evidence['verified'] else 'indexed', 'title': evidence['title_verified'] if evidence and evidence['verified'] and evidence['title_verified'] else row['title'], 'heading_title_verified': bool(evidence and evidence['verified']), 'availability_evidence': evidence.get('availability_evidence', []) if evidence and evidence['verified'] else [], 'implementation_status': 'not_implemented'}
            pages.append(record)
            if evidence and evidence['verified']:
                stack = []
                for j, heading in enumerate(evidence['headings'], 1):
                    section_id = f'{page_id}-H{j:03d}'
                    while stack and stack[-1]['heading_level'] >= heading['level']:
                        stack.pop()
                    section = {'id': section_id, 'vendor': vendor, 'category': row['category'], 'document': record['title'], 'heading': heading['title'], 'heading_level': heading['level'], 'source_line': heading['source_line'], 'source_anchor': heading['source_anchor'], 'parent_section_id': stack[-1]['id'] if stack else None, 'url': row['url'], 'evidence': 'read', 'kind': 'documentation_section', 'implementation_status': 'not_implemented'}
                    options.append(section)
                    stack.append(section)
        vendor_pages = [p for p in pages if p['vendor'] == vendor]
        title = 'Apifox' if vendor == 'apifox' else 'Postman'
        out = [f'# {title} 完整公开目录', '', f'采集日期：2026-10-02。保留官方索引中的全部 {len(catalog)} 个入口；数量是文档入口数，不是独立功能数。', '', '证据：“已读”表示正文采集成功且未截断；“目录”表示仅确认索引有这个入口。Postman 未读取入口的英文名称由 URL 推导，`heading_title_verified=false`，不能当作已核实的官方标题。FAQ、旧版本、供应商账单和 API 端点参考也保留，避免把它们误当成新增功能。', '']
        for category, records in groupby_category(vendor_pages):
            out += [f'## {category or "根目录"}', '', '| ID | 文档 / 入口 | 证据 |', '| --- | --- | --- |']
            out += [f'| {r["id"]} | [{escape(r["title"])}]({r["url"]}) | {"已读" if r["evidence"] == "read" else "目录"} |' for r in records]
            out.append('')
        (DOCS / f'{vendor.upper()}-CATALOG.md').write_text('\n'.join(out))
    package = {'retrieved_at': '2026-10-02', 'method': 'agent-browser official indexes + official documentation', 'catalog_entries': len(pages), 'verified_documents': sum(r['verified'] for r in manifest), 'documentation_section_entries': len(options), 'notice': 'Documentation entries and headings are evidence, not feature counts or implemented functionality.', 'documents': pages, 'sections': options}
    (DOCS / 'feature-options.json').write_text(json.dumps(package, ensure_ascii=False, indent=2))
    with (DOCS / 'feature-options.csv').open('w', newline='') as file:
        fields = ['id','vendor','category','title','url','evidence','heading_title_verified','implementation_status']
        writer = csv.DictWriter(file, fieldnames=fields, extrasaction='ignore'); writer.writeheader(); writer.writerows(pages)
    print(json.dumps({k: package[k] for k in ['catalog_entries','verified_documents','documentation_section_entries']},ensure_ascii=False))


def escape(s):
    return s.replace('|', '\\|').replace('\n', ' ')


def groupby_category(rows):
    groups = collections.OrderedDict()
    for row in rows:
        groups.setdefault(row['category'], []).append(row)
    return groups.items()

if __name__ == '__main__': main()
