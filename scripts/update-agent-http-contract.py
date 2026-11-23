#!/usr/bin/env python3
"""Refresh the two device listeners from a published, revision-bound producer bundle."""
import hashlib
import json
from pathlib import Path
import sys
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = 'https://dev.azure.com/shengming0923/rss/_git/rss-mdm'


def references(value):
    if isinstance(value, dict):
        if 'schemaId' in value:
            yield value['schemaId']
        for child in value.values():
            yield from references(child)
    elif isinstance(value, list):
        for child in value:
            yield from references(child)


def main():
    if len(sys.argv) != 2:
        raise ValueError('usage: update-agent-http-contract.py <published contract ZIP>')
    wire = tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['dependencies']['rss-mdm-agent-wire']
    revision = wire['rev']
    packages = tomllib.loads((ROOT / 'Cargo.lock').read_text())['package']
    sources = [p['source'] for p in packages if p['name'] == 'rss-mdm-agent-wire']
    if wire['git'] != REPOSITORY or len(revision) != 40 or sources != [f'git+{REPOSITORY}?rev={revision}#{revision}']:
        raise ValueError('wire declaration and lock must pin one producer commit')
    endpoints, schemas, sources = [], {}, {}
    with zipfile.ZipFile(sys.argv[1]) as archive:
        manifest = json.loads(archive.read('manifest.json'))
        if manifest['repository'] != 'rss-mdm' or manifest['revision'] != revision:
            raise ValueError('contract differs from locked producer')
        for listener, filename in [('agent', 'rss-mdm-http.json'), ('da', 'rss-da-http.json')]:
            raw = archive.read(filename)
            digest = hashlib.sha256(raw).hexdigest()
            if digest != manifest['sha256'][filename]:
                raise ValueError(f'producer digest mismatch: {filename}')
            sources[listener] = digest
            document = json.loads(raw)
            selected = [e for e in document['endpoints'] if e['listener'] == listener]
            if not selected or document['formatVersion'] != 1 or document['schemaDialect'] != 'https://json-schema.org/draft/2020-12/schema':
                raise ValueError(f'invalid {listener} HTTP contract')
            endpoints.extend(selected)
            for key in set(references(selected)):
                schema = document['schemas'][key]
                if key in schemas and schemas[key] != schema:
                    raise ValueError(f'conflicting schema: {key}')
                schemas[key] = schema
    projection = {'formatVersion': 1, 'schemaDialect': document['schemaDialect'],
                  'endpoints': endpoints, 'schemas': {k: schemas[k] for k in sorted(schemas)}}
    content = (json.dumps(projection, ensure_ascii=False, indent=2) + '\n').encode()
    metadata = {'repository': REPOSITORY, 'revision': revision, 'sources': sources,
                'sha256': hashlib.sha256(content).hexdigest()}
    target = ROOT / 'tests/agent-protocol'
    (target / 'http-v1.json').write_bytes(content)
    (target / 'http-v1-source.json').write_text(json.dumps(metadata, indent=2) + '\n')


if __name__ == '__main__':
    main()
