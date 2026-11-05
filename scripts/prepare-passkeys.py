"""Prepare local signing inputs and the public AASA for the macOS passkey build."""
import argparse
import json
from pathlib import Path
import plistlib
import re
import subprocess
from datetime import datetime, timezone

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("domain", help="HTTPS hostname you control, without scheme or path")
parser.add_argument("profile", type=Path, help="macOS provisioning profile for the app")
parser.add_argument("identity", help="code signing identity authorized by the profile")
args = parser.parse_args()
domain = args.domain.lower()
if not re.fullmatch(r"(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?", domain) or len(domain) > 253:
    parser.error("Expected a fully qualified hostname.")
root = Path(__file__).resolve().parent.parent
bundle = json.loads((root / "src-tauri/tauri.conf.json").read_text())["identifier"]
profile = args.profile.resolve(strict=True)
raw = subprocess.run(["security", "cms", "-D", "-i", str(profile)], check=True, capture_output=True)
data = plistlib.loads(raw.stdout)
ent = data["Entitlements"]
app_id = ent.get("com.apple.application-identifier", "")
team = ent.get("com.apple.developer.team-identifier")
assoc = "com.apple.developer.associated-domains"
allowed = ent.get(assoc, [])
if data["ExpirationDate"].replace(tzinfo=timezone.utc) <= datetime.now(timezone.utc):
    parser.error("The provisioning profile has expired.")
if not team or app_id.partition(".")[2] != bundle or "OSX" not in data.get("Platform", []):
    parser.error("Expected an explicit macOS provisioning profile for " + bundle)
if isinstance(allowed, str):
    allowed = [allowed]
if "*" not in allowed and "webcredentials:" + domain not in allowed:
    parser.error("The provisioning profile does not grant Associated Domains for this hostname.")
out = root / "target/passkeys" / domain
out.mkdir(parents=True, exist_ok=True)
ent_file = out / "entitlements.plist"
ent_file.write_bytes(plistlib.dumps({"com.apple.application-identifier": app_id,
                                     "com.apple.developer.team-identifier": team,
                                     assoc: ["webcredentials:" + domain]}))
aasa = out / ".well-known/apple-app-site-association"
aasa.parent.mkdir(exist_ok=True)
aasa.write_text(json.dumps({"webcredentials": {"apps": [app_id]}}, indent=2) + "\n")
config = out / "tauri.passkeys.json"
config.write_text(json.dumps({"bundle": {"macOS": {"entitlements": str(ent_file),
    "signingIdentity": args.identity, "files": {"embedded.provisionprofile": str(profile)}}}}, indent=2) + "\n")
print(f"Serve {aasa} at https://{domain}/.well-known/apple-app-site-association (application/json, no redirect).")
print("Build configuration:", config)
