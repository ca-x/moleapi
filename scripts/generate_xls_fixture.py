"""Rebuild crates/data/tests/fixtures/workbook.xls with xlwt==1.3.0."""
from pathlib import Path

import xlwt


def sheet(book, name, headings, rows):
    page = book.add_sheet(name)
    for column, value in enumerate(headings):
        page.write(0, column, value)
    for row_index, values in enumerate(rows, 1):
        for column, value in enumerate(values):
            if value is not None:
                page.write(row_index, column, value)
    return page


book = xlwt.Workbook()
sheet(book, "Index", ["marker"], [["first"]])
sheet(
    book,
    "Transactions",
    ["id", "amount", "active", "note", "exact_text", "formula"],
    [
        [2, 2.3, True, "café", "9007199254740993", xlwt.Formula("B2*2")],
        [3, None, False, None, "1.23000000000000000001", None],
    ],
)
sheet(book, "Duplicate Headers", ["name", "name"], [["a", "b"]])
sheet(book, "Blank Headers", ["", "name"], [["a", "b"]])
sheet(book, "Unsafe Number", ["value"], [[9007199254740994]])
blank = book.add_sheet("Leading Blank")
blank.write(0, 1, "name")
blank.write(1, 0, "value")
wide = book.add_sheet("Oversized Grid")
wide.write(0, 0, "first")
wide.write(4000, 255, "last")
output = Path(__file__).resolve().parents[1] / "crates/data/tests/fixtures/workbook.xls"
book.save(str(output))
print(output)
