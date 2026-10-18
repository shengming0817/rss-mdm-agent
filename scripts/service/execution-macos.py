#!/usr/bin/env python3
"""Lab adapter. The installed Rust service owns all installation behavior."""
import argparse
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['install', 'refresh', 'remove', 'remove-lab-registration', 'status', 'apply', 'import'])
    parser.add_argument('--scope', choices=['system', 'user'], default='system')
    parser.add_argument('--binary', type=Path, required=True)
    for name in ['config', 'candidate-binary', 'candidate-config', 'candidate', 'profile', 'configuration', 'package-path']:
        parser.add_argument('--' + name, type=Path)
    args = parser.parse_args()
    if not args.binary.is_absolute():
        parser.error('--binary must be absolute')
    command = [str(args.binary), '--installation', args.action, '--scope', args.scope, '--binary', str(args.binary)]
    for name in ['config', 'candidate_binary', 'candidate_config', 'candidate', 'profile', 'configuration', 'package_path']:
        value = getattr(args, name)
        if value is not None:
            command.extend(['--' + name.replace('_', '-'), str(value)])
    subprocess.run(command, check=True, timeout=240)


if __name__ == '__main__':
    main()
