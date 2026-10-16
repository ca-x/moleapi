#!/usr/bin/env python3
"""Recreate the checked-in XLSX Data fixtures with XlsxWriter 3.2.9."""
from pathlib import Path
from datetime import datetime
from io import BytesIO
from zipfile import ZipFile, ZIP_DEFLATED, ZipInfo
import xlsxwriter

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / 'crates/data/tests/fixtures'
FIXTURES.mkdir(parents=True, exist_ok=True)

workbook = FIXTURES / 'workbook.xlsx'
with xlsxwriter.Workbook(str(workbook)) as book:
    book.set_properties({'created': datetime(2020, 1, 1)})
    overview = book.add_worksheet('Overview')
    overview.write_row(0, 0, ['marker'])
    overview.write(1, 0, 'first-sheet')

    sheet = book.add_worksheet('Transactions')
    sheet.write_row(0, 0, ['id', 'amount', 'active', 'label', 'exact_text', 'calculated', 'when'])
    sheet.write_number(1, 0, 1)
    sheet.write_number(1, 1, 1.23)
    sheet.write_boolean(1, 2, True)
    sheet.write_string(1, 3, 'first')
    sheet.write_string(1, 4, '9007199254740993')
    sheet.write_formula(1, 5, '=B2*2', None, 2.46)
    sheet.write_datetime(1, 6, datetime(2024, 2, 29), book.add_format({'num_format': 'yyyy-mm-dd'}))
    sheet.write_number(2, 0, 2)
    sheet.write_number(2, 1, 2.5)
    sheet.write_boolean(2, 2, False)
    sheet.write_string(2, 4, '1.23000000000000000001')

    duplicate = book.add_worksheet('Duplicate Headers')
    duplicate.write_row(0, 0, ['same', 'same'])
    duplicate.write_row(1, 0, [1, 2])
    case_collision = book.add_worksheet('Case Colliding Headers')
    case_collision.write_row(0, 0, ['Name', 'name'])
    case_collision.write_row(1, 0, ['first', 'second'])
    blank = book.add_worksheet('Blank Headers')
    blank.write(0, 0, 'first')
    blank.write(1, 0, 1)
    blank.write(1, 1, 2)
    errors = book.add_worksheet('Error Cells')
    errors.write(0, 0, 'value')
    errors.write_formula(1, 0, '=1/0', None, '#DIV/0!')
    leading = book.add_worksheet('Leading Blank')
    leading.write_row(1, 0, ['id', 'name'])
    leading.write_row(2, 0, [1, 'first'])

def fixed_entry(name):
    entry = ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
    entry.compress_type = ZIP_DEFLATED
    return entry


normalized = BytesIO()
with ZipFile(workbook) as source, ZipFile(normalized, 'w') as output:
    for item in source.infolist():
        output.writestr(fixed_entry(item.filename), source.read(item.filename))
workbook.write_bytes(normalized.getvalue())

with ZipFile(FIXTURES / 'oversized-entry.xlsx', 'w') as archive:
    archive.writestr(fixed_entry('xl/worksheets/sheet1.xml'), b'A' * (65 * 1024 * 1024))
