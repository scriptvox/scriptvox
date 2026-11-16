#!/usr/bin/env python3
"""HEAD every pinned URL in toolchain.lock: a removed file is found here, not at a user's first setup."""
import sys
import tomllib
import urllib.error
import urllib.request

bad = 0
for t in tomllib.load(open("toolchain.lock", "rb"))["tool"]:
    if not t["url"]:
        print(f"- {t['name']}: no address yet (setup takes --from)")
        continue
    req = urllib.request.Request(t["url"], method="HEAD", headers={"User-Agent": "scriptvox-pins-check"})
    try:
        urllib.request.urlopen(req, timeout=20).read(0)
    except urllib.error.HTTPError as e:
        if e.code in (404, 410):
            print(f"GONE {t['name']}: HTTP {e.code} {t['url']}")
            bad += 1
        else:
            print(f"? {t['name']}: HTTP {e.code}")
    except Exception as e:
        print(f"? {t['name']}: {e}")
sys.exit(1 if bad else 0)
