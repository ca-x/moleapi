#!/usr/bin/env python3
"""Validate published research consistency and links without refetching the web."""
from pathlib import Path
from urllib.parse import urlparse
import collections, csv, json, re

ROOT=Path(__file__).resolve().parents[1]
DOCS=ROOT/'docs'
REF=DOCS/'references'


def require(condition,message):
    if not condition: raise ValueError(message)


def norm(url):
    p=urlparse(url);return p.netloc+p.path.rstrip('/').removesuffix('.md')


def main():
    package=json.loads((DOCS/'feature-options.json').read_text())
    requirements=json.loads((DOCS/'features.json').read_text())
    manifest=json.loads((REF/'fetch-manifest.json').read_text())
    catalogs=[json.loads((REF/f'{vendor}-catalog.json').read_text()) for vendor in ['apifox','postman']]
    document_count=sum(map(len,catalogs))
    fox_source=(REF/'apifox-llms.txt').read_text()
    expected_fox=set(re.findall(r'\]\((https://docs\.apifox\.com/[^)]+)\)',fox_source))
    require(expected_fox=={r['url'] for r in catalogs[0]},'Apifox source index lost entries')
    import xml.etree.ElementTree as ET
    sitemap=ET.parse(REF/'postman-sitemap.xml').getroot()
    expected_postman={e.text for e in sitemap.iter('{http://www.sitemaps.org/schemas/sitemap/0.9}loc')}
    require(expected_postman=={r['url'] for r in catalogs[1]},'Postman source sitemap lost entries')
    require(package['catalog_entries']==len(package['documents'])==document_count,'catalog completeness mismatch')
    require(len({d['id'] for d in package['documents']})==document_count,'duplicate document IDs')
    require(len({d['url'] for d in package['documents']})==document_count,'duplicate exact source URLs')
    require(package['verified_documents']==sum(x['verified'] for x in manifest),'verified document count mismatch')
    require(package['documentation_section_entries']==len(package['sections']),'section count mismatch')
    require(len({s['id'] for s in package['sections']})==len(package['sections']),'duplicate section IDs')
    indexed={norm(d['url']) for d in package['documents']}
    read={norm(d['url']) for d in manifest if d['verified']}
    for document in package['documents']:
        require(document['implementation_status']=='not_implemented','unverified implementation claim')
        require(document['evidence'] in ['read','indexed'],'unknown evidence status')
        if document['evidence']=='read':require(norm(document['url']) in read,'false body-read claim')
    sections={s['id']:s for s in package['sections']}
    for section in package['sections']:
        require(norm(section['url']) in read,'section not backed by verified source')
        require(section['heading_level'] in range(1,7),'invalid heading level')
        require(section['source_line']>=1,'invalid source line')
        parent=section['parent_section_id']
        if parent:
            require(parent in sections,'missing heading parent')
            require(sections[parent]['heading_level']<section['heading_level'],'invalid parent heading level')
            require(sections[parent]['url']==section['url'],'heading parent in different document')
    module_ids={m['id'] for m in requirements['modules']}
    require(len(module_ids)==len(requirements['modules']),'duplicate module IDs')
    require(len({f['id'] for f in requirements['features']})==len(requirements['features']),'duplicate feature IDs')
    graph={m['id']:m['depends_on'] for m in requirements['modules']}
    visited=set();active=set()
    def visit(node):
        require(node in module_ids,f'unknown module {node}')
        require(node not in active,f'cyclic module dependency at {node}')
        if node in visited:return
        active.add(node)
        for dep in graph[node]:visit(dep)
        active.remove(node);visited.add(node)
    for node in graph:visit(node)
    for feature in requirements['features']:
        require(feature['module'] in module_ids,'unknown feature module')
        require(feature['status']=='pending','false completion status')
        for url in feature['apifox_sources']+feature['postman_sources']:
            require(norm(url) in indexed,'source absent from official catalog: '+url)
            require(feature['evidence'][url]==('read' if norm(url) in read else 'indexed'),'incorrect evidence badge')
    controls=json.loads((DOCS/'configuration-options.json').read_text())['controls']
    require(len({c['id'] for c in controls})==len(controls),'duplicate control IDs')
    for control in controls:
        require(norm(control['source_url']) in read,'concrete control lacks read source')
        require(control['implementation_status']=='not_implemented','false control implementation claim')
        require(bool(control['options']),'empty control options')
    for path,key in [('feature-options.csv','documents'),('features.csv','features')]:
        with (DOCS/path).open(newline='') as file:rows=list(csv.DictReader(file))
        expected=package[key] if key=='documents' else requirements[key]
        require(len(rows)==len(expected),f'CSV count mismatch: {path}')
    for path in [ROOT/'README.md', *[p for p in DOCS.glob('*.md') if p.name not in ['INITIAL-SPEC.md','API-CONTRACT.md']]]:
        text=path.read_text()
        for link in re.findall(r'\]\(([^)]+)\)',text):
            if link.startswith(('https://','http://','#')):continue
            target=(path.parent/link.split('#')[0]).resolve()
            require(target.exists(),f'broken local link in {path.name}: {link}')
    counts=dict(collections.Counter(d['vendor'] for d in manifest if d['verified']))
    print(json.dumps({'result':'pass','catalog_entries':document_count,'verified_document_bodies':counts,'documentation_sections':len(package['sections']),'modules':len(module_ids),'planned_features':len(requirements['features'])},ensure_ascii=False))

if __name__=='__main__':main()
