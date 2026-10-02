"""CommonMark headings, excluding code fences and preserving hierarchy."""
from markdown_it import MarkdownIt


def extract_headings(content):
    tokens = MarkdownIt('commonmark').parse(content)
    headings = []
    for index, token in enumerate(tokens):
        if token.type == 'heading_open' and index + 1 < len(tokens):
            inline = tokens[index + 1]
            title = ''.join(child.content for child in (inline.children or []) if child.type in {'text', 'code_inline', 'image'}).strip()
            if title:
                headings.append({'title': title, 'level': int(token.tag[1:]), 'source_line': token.map[0] + 1, 'source_anchor': None})
    return headings
