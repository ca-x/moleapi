"""Build the small ODS fixture consumed by the real Calamine/DataFusion tests."""

from pathlib import Path
from zipfile import ZIP_DEFLATED, ZIP_STORED, ZipFile, ZipInfo


ROOT = Path(__file__).resolve().parents[2]
DEST = ROOT / "crates/data/tests/fixtures/workbook.ods"
BAD_MIMETYPE_DEST = ROOT / "crates/data/tests/fixtures/invalid-mimetype.ods"
MIMETYPE = "application/vnd.oasis.opendocument.spreadsheet"


def cell(kind: str, value: str = "", formula: str = "") -> str:
    if kind == "empty":
        return "<table:table-cell/>"
    attributes = {"string": "office:string-value", "float": "office:value", "boolean": "office:boolean-value"}
    extra = f' table:formula="{formula}"' if formula else ""
    return f'<table:table-cell office:value-type="{kind}" {attributes[kind]}="{value}"{extra}/>'


def row(*cells: str) -> str:
    return "<table:table-row>" + "".join(cells) + "</table:table-row>"


def sheet(name: str, *rows: str) -> str:
    return f'<table:table table:name="{name}">' + "".join(rows) + "</table:table>"


headings = lambda *names: row(*(cell("string", name) for name in names))
content = (
    '<?xml version="1.0" encoding="UTF-8"?>'
    '<office:document-content '
    'xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" '
    'xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" '
    'xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" office:version="1.2">'
    '<office:body><office:spreadsheet>'
    + sheet("Overview", headings("marker"), row(cell("string", "first")))
    + sheet(
        "Transactions",
        headings("id", "amount", "active", "note", "formula"),
        row(cell("float", "2"), cell("float", "2.30"), cell("boolean", "true"), cell("string", "caf&#233;"), cell("float", "4.6", "of:=B2*2")),
        row(cell("float", "3"), cell("empty"), cell("boolean", "false"), cell("string", "second"), cell("empty")),
    )
    + sheet("Duplicate Headers", headings("id", "ID"), row(cell("float", "1"), cell("float", "2")))
    + sheet("Blank Headers", row(cell("string", "id"), cell("empty")), row(cell("float", "1"), cell("float", "2")))
    + sheet("Leading Blank", row(cell("empty")), headings("value"), row(cell("string", "after")))
    + sheet("Unsafe Number", headings("value"), row(cell("float", "9007199254740993")))
    + sheet("Exact Text", headings("value"), row(cell("string", "9007199254740993")))
    + "</office:spreadsheet></office:body></office:document-content>"
)
manifest = (
    '<?xml version="1.0" encoding="UTF-8"?>'
    '<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2">'
    f'<manifest:file-entry manifest:full-path="/" manifest:media-type="{MIMETYPE}"/>'
    '<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>'
    '</manifest:manifest>'
)


def info(name: str, compression: int) -> ZipInfo:
    entry = ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
    entry.compress_type = compression
    return entry


with ZipFile(DEST, "w") as archive:
    archive.writestr(info("mimetype", ZIP_STORED), MIMETYPE)
    archive.writestr(info("content.xml", ZIP_DEFLATED), content)
    archive.writestr(info("META-INF/manifest.xml", ZIP_DEFLATED), manifest)

with ZipFile(BAD_MIMETYPE_DEST, "w") as archive:
    archive.writestr(info("mimetype", ZIP_STORED), MIMETYPE[:-1] + "x")
    archive.writestr(info("content.xml", ZIP_DEFLATED), content)
