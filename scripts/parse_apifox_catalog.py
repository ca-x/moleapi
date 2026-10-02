#!/usr/bin/env python3
"""Read the entire official Markdown index using a CommonMark parser."""
from pathlib import Path
from urllib.parse import urlparse
from markdown_it import MarkdownIt
import json,re
ROOT=Path(__file__).resolve().parents[1]
ref=ROOT/'docs/references'
content=(ref/'apifox-llms.txt').read_text()
rows=[]
for token in MarkdownIt('commonmark').parse(content):
    if token.type!='inline':continue
    children=token.children or []
    for i,child in enumerate(children):
        if child.type!='link_open':continue
        url=child.attrGet('href')
        if not url or urlparse(url).hostname!='docs.apifox.com':continue
        before=''.join(x.content for x in children[:i] if x.type in {'text','code_inline'}).strip()
        end=i+1
        while end<len(children) and children[end].type!='link_close':end+=1
        title=''.join(x.content for x in children[i+1:end] if x.type in {'text','code_inline'}).strip()
        description=''.join(x.content for x in children[end+1:] if x.type in {'text','code_inline'}).strip().removeprefix(':').strip()
        rows.append({'category':before,'title':title,'url':url,'description':description})
expected=re.findall(r'\]\((https://docs\.apifox\.com/[^)]+)\)',content)
if set(expected)!={x['url'] for x in rows}:
    raise ValueError('CommonMark parsing must preserve every official URL, including IPv6 brackets in titles')
(ref/'apifox-catalog.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2))
print(f'Apifox: {len(rows)} entries parsed; IPv6 title brackets preserved')
