#!/usr/bin/env python3
"""Host-only syntax and pure-function checks; never invokes Windows collection APIs."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import export


def main():
    pwsh = os.environ.get('COLLECTION_PWSH') or shutil.which('pwsh')
    if not pwsh or not Path(pwsh).is_absolute():
        raise SystemExit('Set COLLECTION_PWSH to the absolute host PowerShell 7.4.13 binary; Windows tools are not required')
    version = subprocess.check_output(
        [pwsh, '-NoLogo', '-NoProfile', '-NonInteractive', '-Command', '$PSVersionTable.PSVersion.ToString()'],
        text=True, timeout=15).strip()
    if version != '7.4.13':
        raise SystemExit(f'Expected the pinned PowerShell 7.4.13 test candidate, got {version}')
    with tempfile.TemporaryDirectory(prefix='rss-collection-') as directory:
        for name, content in export.resources():
            Path(directory, name).write_bytes(content)
        subprocess.run([pwsh, '-NoLogo', '-NoProfile', '-NonInteractive', '-File',
                        str(export.ROOT / 'test.ps1'), '-Resources', directory], check=True, timeout=120)


if __name__ == '__main__':
    main()
