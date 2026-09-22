#!/usr/bin/env python3
"""Revoke local query access or uninstall the laboratory service; never touches AI data."""
import argparse
import json
import os
from pathlib import Path
import subprocess
from importlib.util import spec_from_file_location, module_from_spec

spec = spec_from_file_location("installer", Path(__file__).with_name("install-macos.py"))
installer = module_from_spec(spec)
spec.loader.exec_module(installer)
parser = argparse.ArgumentParser(description=__doc__)
action = parser.add_mutually_exclusive_group(required=True)
action.add_argument("--revoke-user")
action.add_argument("--uninstall", action="store_true")
args = parser.parse_args()
if os.geteuid() != 0:
    raise RuntimeError("administrator required")
installer.stop()
if args.uninstall:
    installer.PLIST.unlink(missing_ok=True)
    print("Service removed from launchd; retained installation and AI data are unchanged.")
else:
    path = installer.ROOT / "policy.json"
    policy = json.loads(path.read_text())
    policy["allowed_users"] = [u for u in policy["allowed_users"] if u != args.revoke_user]
    temporary = path.with_suffix(".new")
    temporary.write_text(json.dumps(policy, indent=2) + "\n")
    os.chown(temporary, 0, 0)
    os.chmod(temporary, 0o644)
    temporary.replace(path)
    if policy["allowed_users"]:
        installer.run("/bin/launchctl", "bootstrap", "system", str(installer.PLIST))
    print("Old connections invalidated; revoked user cannot establish a new query.")
