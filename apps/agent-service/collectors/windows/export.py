#!/usr/bin/env python3
"""Produce two immutable, standalone Script Task resources from their sole sources."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SECTIONS = 'identity,os,cpu,memory,firmware,disks,volumes,battery,network'.split(',')


def resources():
    common = (ROOT / 'common.ps1').read_text()
    for name in ('hardware', 'software'):
        section = ''
        if name == 'hardware':
            choices = ','.join(repr(s) for s in SECTIONS)
            section = f'[Parameter(Mandatory)][ValidateSet({choices})][string]$Section,\n'
        header = ('#requires -Version 7.4\n[CmdletBinding()]\nparam(\n' + section +
                  '[ValidateRange(1,65535)][int]$MaxItems=10000,\n'
                  '[ValidateRange(1024,16777216)][int]$MaxBytes=8388608,\n'
                  '[ValidateRange(5,3600)][int]$TimeoutSeconds=120\n)\n')
        body = (header + common + '\n' + (ROOT / f'{name}.ps1').read_text()).encode()
        yield f'windows-{name}-v1.ps1', body


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    entries = []
    for name, body in resources():
        (args.output / name).write_bytes(body)
        entries.append({'file': name, 'bytes': len(body), 'sha256': hashlib.sha256(body).hexdigest(),
                        'profile': 'power_shell7', 'runAs': 'system'})
    (args.output / 'manifest.json').write_text(json.dumps({'format': 1, 'resources': entries}, indent=2) + '\n')


if __name__ == '__main__':
    main()
