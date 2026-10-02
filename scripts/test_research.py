import unittest
from markdown_it import MarkdownIt
from markdown_headings import extract_headings


class ResearchParsingTests(unittest.TestCase):
    def test_code_comments_do_not_become_documentation_options(self):
        doc = '# Real API\n\n```python\n# fake option\n## another fake\n```\n\n## Genuine setting\n'
        headings = extract_headings(doc)
        self.assertEqual([h['title'] for h in headings], ['Real API', 'Genuine setting'])
        self.assertEqual([h['level'] for h in headings], [1, 2])
        self.assertEqual(headings[1]['source_line'], 8)

    def test_setext_headings_and_inline_format_are_preserved(self):
        doc = 'Parent\n======\n\n### Use **API** `token`\n'
        headings = extract_headings(doc)
        self.assertEqual([(h['title'], h['level']) for h in headings], [('Parent', 1), ('Use API token', 3)])

    def test_ipv6_brackets_do_not_hide_official_links(self):
        text = '- 常见问题 [如何请求 http://[::1]？](https://docs.apifox.com/6888333m0.md): IPv6\n'
        links = [child.attrGet('href') for token in MarkdownIt().parse(text) for child in (token.children or []) if child.type == 'link_open']
        self.assertEqual(links, ['https://docs.apifox.com/6888333m0.md'])


if __name__ == '__main__':
    unittest.main()
