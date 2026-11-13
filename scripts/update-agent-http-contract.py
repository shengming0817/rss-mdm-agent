#!/usr/bin/env python3
"""Refresh Agent test inputs during an explicit fixed-wire upgrade only."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = 'https://dev.azure.com/shengming0923/rss/_git/rss-mdm'


def run(*args, cwd):
    return subprocess.check_output(args, cwd=cwd)


def main():
    if len(sys.argv) != 2:
        raise ValueError('usage: update-agent-http-contract.py <clean backend checkout>')
    source = Path(sys.argv[1]).resolve()
    metadata = json.loads(run('cargo', 'metadata', '--locked', '--all-features', '--format-version', '1', cwd=ROOT))
    packages = [p for p in metadata['packages'] if p['name'] == 'rss-mdm-agent-wire']
    if len(packages) != 1:
        raise ValueError('expected one fixed wire package')
    origin = packages[0]['source']
    prefix = 'git+' + REPOSITORY + '?rev='
    if not origin.startswith(prefix):
        raise ValueError('unexpected wire source')
    revision, precise = origin[len(prefix):].split('#')
    if revision != precise or len(revision) != 40:
        raise ValueError('wire needs one complete commit')

    def clean():
        if run('/usr/bin/git', 'rev-parse', 'HEAD', cwd=source).decode().strip() != revision:
            raise ValueError('backend checkout differs from the locked wire commit')
        if run('/usr/bin/git', 'status', '--porcelain', cwd=source):
            raise ValueError('backend checkout must be clean throughout export')

    clean()
    with tempfile.TemporaryDirectory(prefix='agent-http-export-') as temporary:
        exported = Path(temporary) / 'http-v1.json'
        subprocess.run([sys.executable, str(source / 'hack/http_contract_artifact.py'), '--output', str(exported)], cwd=source, check=True)
        raw = exported.read_bytes()
    clean()
    document = json.loads(raw)
    endpoints = [e for e in document['endpoints'] if e['listener'] == 'agent']
    if document['formatVersion'] != 1 or not endpoints:
        raise ValueError('expected nonempty Agent HTTP V1')
    needed = set()

    def references(value):
        if isinstance(value, dict):
            if 'schemaId' in value:
                needed.add(value['schemaId'])
            for child in value.values():
                references(child)
        elif isinstance(value, list):
            for child in value:
                references(child)

    references(endpoints)
    selected = {'formatVersion': document['formatVersion'], 'schemaDialect': document['schemaDialect'],
                'endpoints': endpoints, 'schemas': {key: document['schemas'][key] for key in sorted(needed)}}
    content = (json.dumps(selected, ensure_ascii=False, indent=2) + '\n').encode()
    provenance = {'repository': REPOSITORY, 'revision': revision,
                  'generator': 'python3 hack/http_contract_artifact.py --output <file>',
                  'listener': 'agent', 'sourceSha256': hashlib.sha256(raw).hexdigest(),
                  'sha256': hashlib.sha256(content).hexdigest()}
    destination = ROOT / 'tests/agent-protocol'
    (destination / 'http-v1.json').write_bytes(content)
    (destination / 'http-v1-source.json').write_text(json.dumps(provenance, indent=2) + '\n')
    print(f'Agent HTTP snapshot: {revision}, {len(endpoints)} endpoints, {len(needed)} schemas')


if __name__ == '__main__':
    main()
