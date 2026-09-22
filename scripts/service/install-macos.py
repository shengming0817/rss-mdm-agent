#!/usr/bin/env python3
"""Administrator-only lab install. Stops the old service before changing trust."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import pwd
import re
import shutil
import subprocess
import uuid

ROOT = Path("/Library/Application Support/RSS MDM Agent/service")
APP = Path("/Applications/RSS MDM Agent.app")
LABEL = "com.rss-mdm.agent.status"
PLIST = Path("/Library/LaunchDaemons") / (LABEL + ".plist")
ACCOUNT = "_rssmdmstatus"

def run(*args):
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout

def artifact(path):
    details = subprocess.run(["/usr/bin/codesign", "-dv", "--verbose=4", str(path)],
                             check=True, capture_output=True, text=True).stderr
    match = re.search(r"^CDHash=([a-f0-9]{40})$", details, re.M)
    if not match:
        raise RuntimeError("fixed code-directory identity unavailable")
    return dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest(), cdhash=match[1])

def stop():
    loaded = subprocess.run(["/bin/launchctl", "print", "system/" + LABEL], capture_output=True).returncode == 0
    if loaded:
        run("/bin/launchctl", "bootout", "system/" + LABEL)
    if subprocess.run(["/bin/launchctl", "print", "system/" + LABEL], capture_output=True).returncode == 0:
        raise RuntimeError("old service did not stop")

def protect(root):
    for directory, dirs, files in os.walk(root, followlinks=False):
        for path in [Path(directory), *(Path(directory) / p for p in dirs + files)]:
            if path.is_symlink():
                os.chown(path, 0, 0, follow_symlinks=False)
            else:
                executable = path.is_dir() or bool(path.stat().st_mode & 0o111)
                os.chown(path, 0, 0)
                os.chmod(path, 0o755 if executable else 0o644)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--desktop", type=Path, required=True)
    parser.add_argument("--service", type=Path, required=True)
    parser.add_argument("--allow-user", action="append", required=True)
    args = parser.parse_args()
    if os.geteuid() != 0:
        raise RuntimeError("run this installer as administrator")
    if os.uname().machine != "arm64" or tuple(map(int, run("/usr/bin/sw_vers", "-productVersion").strip().split(".")[:2])) < (26, 4):
        raise RuntimeError("requires macOS 26.4+ arm64")
    users = [str(pwd.getpwuid(int(uid)).pw_uid) for uid in args.allow_user]
    if "0" in users:
        raise RuntimeError("ordinary user required")
    if not args.desktop.is_dir() or not args.service.is_file():
        raise RuntimeError("complete fixed desktop and service artifacts required")
    stop()
    try:
        account = pwd.getpwnam(ACCOUNT)
    except KeyError:
        used = {p.pw_uid for p in pwd.getpwall()}
        uid = next(uid for uid in range(400, 500) if uid not in used)
        for key, value in [("UniqueID", str(uid)), ("PrimaryGroupID", "20"),
                           ("UserShell", "/usr/bin/false"), ("NFSHomeDirectory", "/var/empty"),
                           ("IsHidden", "1"), ("RealName", "RSS status service")]:
            run("/usr/bin/dscl", ".", "-create", "/Users/" + ACCOUNT, key, value)
        account = pwd.getpwnam(ACCOUNT)
    ROOT.mkdir(parents=True, exist_ok=True)
    installation = str(uuid.uuid4())
    old = ROOT / "policy.json"
    if old.exists():
        installation = json.loads(old.read_text())["installation"]
    staged = APP.with_name("RSS MDM Agent.staged.app")
    if staged.exists():
        raise RuntimeError("staged application already exists; inspect it before retrying")
    shutil.copytree(args.desktop, staged, symlinks=True)
    run("/usr/bin/codesign", "--force", "--options", "runtime", "--sign", "-", str(staged))
    run("/usr/bin/codesign", "--verify", "--strict", str(staged))
    protect(staged)
    if APP.exists():
        raise RuntimeError("remove the previous lab application explicitly before reinstalling")
    staged.rename(APP)
    service = ROOT / "rss-local-service"
    shutil.copy2(args.service, service)
    run("/usr/bin/codesign", "--force", "--options", "runtime", "--sign", "-", str(service))
    with (APP / "Contents/Info.plist").open("rb") as file:
        executable = plistlib.load(file)["CFBundleExecutable"]
    client = APP / "Contents/MacOS" / executable
    policy = dict(version=1, installation=installation, build=hashlib.sha256(service.read_bytes()).hexdigest(),
                  platform="macos-arm64", service_subject=str(account.pw_uid), allowed_users=users,
                  client=artifact(client), service=artifact(service))
    temporary = ROOT / "policy.new"
    temporary.write_text(json.dumps(policy, indent=2) + "\n")
    protect(ROOT.parent)
    temporary.replace(old)
    value = dict(Label=LABEL, ProgramArguments=[str(service)], UserName=ACCOUNT,
                 MachServices={LABEL: True}, RunAtLoad=True, KeepAlive=True,
                 ProcessType="Background")
    with PLIST.open("wb") as file:
        plistlib.dump(value, file)
    os.chown(PLIST, 0, 0)
    os.chmod(PLIST, 0o644)
    run("/bin/launchctl", "bootstrap", "system", str(PLIST))
    print("Installed laboratory status service; platform acceptance remains required.")

if __name__ == "__main__":
    main()
