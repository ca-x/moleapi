#!/usr/bin/env python3
"""Execute original generated DDL in a fresh, owned native QA database."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
from urllib.parse import parse_qs, unquote, urlparse
import uuid

NOTE = '鼹鼠 "quotes" & +'
CHILD = json.dumps({'id': 2, 'status': 'done', 'note': NOTE}, ensure_ascii=False)


def verify(args):
    root = args.fixtures / args.target
    assert args.fixtures.is_absolute() and root.is_dir()
    manifest = json.loads((root / 'moleapi-generation.json').read_text())
    assert manifest['target'] == args.target and manifest['engine'] == 'openapi-generator@7.26.0'
    source = (root / 'fixture-source.json').read_bytes()
    assert json.loads(source)['openapi'] == args.dialect
    assert hashlib.sha256(source).hexdigest() == manifest['source_sha256']
    before = {file['path']: hashlib.sha256((root / file['path']).read_bytes()).hexdigest() for file in manifest['files']}
    assert all(before[file['path']] == file['sha256'] for file in manifest['files'])
    evidence_path = root / 'runtime-evidence.json'
    assert not evidence_path.exists(), 'Use a fresh fixture root'
    database = 'moleapi_model_qa_' + uuid.uuid4().hex
    postgres = args.target == 'postgresql-schema'
    if postgres:
        import psycopg
        from psycopg import sql
        from psycopg.conninfo import conninfo_to_dict
        settings = conninfo_to_dict(os.environ[args.url_env])
        settings['connect_timeout'] = 15
        admin = psycopg.connect(**settings, autocommit=True)
        create = sql.SQL('CREATE DATABASE {} ENCODING \'UTF8\' TEMPLATE template0').format(sql.Identifier(database))
        drop = sql.SQL('DROP DATABASE {}').format(sql.Identifier(database))
        settings['dbname'] = database
        connect = lambda: psycopg.connect(**settings, autocommit=True)
        table = 'pet'
        ddl_name = 'postgresql_schema.sql'
        invalid_error = psycopg.Error
    else:
        import pymysql
        parsed = urlparse(os.environ[args.url_env])
        assert parsed.scheme == 'mysql'
        settings = {'host': parsed.hostname or 'localhost', 'port': parsed.port or 3306,
                    'user': unquote(parsed.username or ''), 'password': unquote(parsed.password or ''),
                    'charset': 'utf8mb4', 'autocommit': True, 'connect_timeout': 15,
                    'client_flag': pymysql.constants.CLIENT.MULTI_STATEMENTS}
        query = parse_qs(parsed.query)
        if 'unix_socket' in query:
            settings['unix_socket'] = query['unix_socket'][0]
        admin = pymysql.connect(**settings)
        create = f'CREATE DATABASE `{database}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci'
        drop = f'DROP DATABASE `{database}`'
        settings['database'] = database
        connect = lambda: pymysql.connect(**settings)
        table = 'Pet'
        ddl_name = 'mysql_schema.sql'
        invalid_error = pymysql.Error
    created = False
    try:
        with admin.cursor() as cursor:
            cursor.execute(create)
            created = True
        connection = connect()
        try:
            with connection.cursor() as cursor:
                cursor.execute('SELECT version()')
                version = cursor.fetchone()[0]
                if not postgres:
                    cursor.execute("SET SESSION sql_mode='STRICT_ALL_TABLES,NO_ENGINE_SUBSTITUTION,NO_AUTO_VALUE_ON_ZERO'")
                cursor.execute((root / ddl_name).read_text())
                while cursor.nextset():
                    pass
                cursor.execute(f'INSERT INTO {table} (id,status,note,friend) VALUES (%s,%s,%s,%s)', (0, 'ready', None, CHILD))
                cursor.execute(f'INSERT INTO {table} (id,status,note) VALUES (%s,%s,%s)', (2, 'done', NOTE))
                cursor.execute(f'SELECT id,status,note,friend FROM {table} ORDER BY id')
                rows = cursor.fetchall()
                assert len(rows) == 2
                assert tuple(rows[0][:3]) == (0, 'ready', None)
                assert json.loads(rows[0][3]) == json.loads(CHILD)
                assert tuple(rows[1]) == (2, 'done', NOTE, None)
                schema = 'public' if postgres else database
                cursor.execute('SELECT column_name,data_type,is_nullable FROM information_schema.columns WHERE table_schema=%s AND table_name=%s ORDER BY ordinal_position', (schema, table))
                columns = {name: (kind.lower(), nullable) for name, kind, nullable in cursor.fetchall()}
                assert columns['id'][1] == 'NO' and columns['status'][1] == 'NO'
                assert columns['note'][1] == 'YES' and columns['friend'][0] == 'text'
                assert columns['status'][0] == ('user-defined' if postgres else 'enum')
                for statement, values, expected_error in [
                    (f'INSERT INTO {table} (id,status) VALUES (%s,%s)', (3, 'invalid'), '22P02' if postgres else 1265),
                    (f'INSERT INTO {table} (status) VALUES (%s)', ('ready',), '23502' if postgres else 1364),
                ]:
                    try:
                        cursor.execute(statement, values)
                    except invalid_error as error:
                        assert (error.sqlstate if postgres else error.args[0]) == expected_error
                    else:
                        raise AssertionError('Native enum/required constraint was not enforced')
                cursor.execute(f'SELECT COUNT(*) FROM {table}')
                assert cursor.fetchone()[0] == 2
        finally:
            connection.close()
    finally:
        try:
            if created:
                with admin.cursor() as cursor:
                    cursor.execute(drop)
        finally:
            admin.close()
    assert all(hashlib.sha256((root / path).read_bytes()).hexdigest() == digest for path, digest in before.items())
    evidence = {'target': args.target, 'dialect': args.dialect, 'engine': manifest['engine'],
                'database_version': version, 'platform': platform.platform(),
                'source_sha256': manifest['source_sha256'], 'generated_source_sha256': before,
                'columns': columns, 'profiles': ['original-ddl', 'native-enum', 'zero', 'null', 'unicode',
                                               'nested-json-text', 'invalid-enum', 'required-id', 'owned-database-cleanup'],
                'result': 'passed'}
    evidence_path.write_text(json.dumps(evidence, indent=2) + '\n')
    print(f'PASS {args.target}/{args.dialect}: {version}; original DDL and native constraints')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixtures', required=True, type=Path)
    parser.add_argument('--dialect', required=True, choices=['3.0.3', '3.1.0'])
    parser.add_argument('--target', required=True, choices=['postgresql-schema', 'mysql-schema'])
    parser.add_argument('--url-env', required=True, help='Name of environment variable containing an explicit QA admin URL')
    verify(parser.parse_args())
