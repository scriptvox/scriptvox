#!/usr/bin/env python3
"""skills_check.py: checks every skill in plugin/skills/ before a commit.

1. Frontmatter: `name` equals the folder name, `description` is present and at most 1024 characters.
2. Size: SKILL.md at most 300 lines (detail goes in references/).
3. Commands: every `scriptvox <sub>` named in the skill is a real subcommand, and every `--flag` written after it in the
   same command is a real flag of that subcommand (read from `scriptvox <sub> --help`).
4. Originality: no run of 8 words is shared with any skill text of the studied projects (../repos). This is our proof
   that a skill was written from our own notes, not copied or closely paraphrased. Skipped with a notice when ../repos
   is absent (a fresh clone).

Usage: scripts/skills_check.py [--bin target/debug/scriptvox] [--repos ../repos]. Exit 1 on any failure.
"""
import argparse, pathlib, re, subprocess, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
N = 8
SOURCE_GLOBS = ["**/skills/**/*.md", "**/.claude/skills/**/*.md", "**/.agents/skills/**/*.md", "**/SKILL.md"]


def words(text):
    return re.findall(r"[a-z0-9]+(?:'[a-z]+)?", text.lower())


def grams(text):
    w = words(text)
    return {" ".join(w[i:i + N]) for i in range(len(w) - N + 1)}


def source_grams(repos):
    seen, out = set(), set()
    for g in SOURCE_GLOBS:
        for f in repos.glob(g):
            if f in seen or not f.is_file() or "node_modules" in f.parts:
                continue
            seen.add(f)
            out |= grams(f.read_text(errors="ignore"))
    return out, len(seen)


def frontmatter(text):
    m = re.match(r"---\n(.*?)\n---\n", text, re.S)
    if not m:
        return None
    fm = {}
    for line in m.group(1).splitlines():
        k, _, v = line.partition(":")
        fm[k.strip()] = v.strip()
    return fm


def subcommands(binary):
    out = subprocess.run([binary, "--help"], capture_output=True, text=True).stdout
    sect = out.split("Commands:", 1)[1].split("Options:", 1)[0]
    names = {line.split()[0] for line in sect.splitlines() if line.strip()}
    for a in re.findall(r"\[(?:visible )?alias(?:es)?: ([^\]]+)\]", sect):
        names |= {x.strip() for x in a.split(",")}
    return names - {"help"}


def flags(binary, sub, cache={}):
    if sub not in cache:
        out = subprocess.run([binary, sub, "--help"], capture_output=True, text=True).stdout
        found = set(re.findall(r"--[a-z][a-z0-9-]*", out)) | {"--json", "--help"}
        # The flags of its own subcommands too: `scriptvox voices add --sample`.
        part = re.search(r"^Commands:\n(.*?)(?:\n\n|\Z)", out, re.S | re.M)
        for line in part.group(1).splitlines() if part else []:
            name = line.split()[0] if line.startswith("  ") and line.split() else ""
            if name and name != "help":
                more = subprocess.run([binary, sub, name, "--help"], capture_output=True, text=True).stdout
                found |= set(re.findall(r"--[a-z][a-z0-9-]*", more))
        cache[sub] = found
    return cache[sub]


def check_commands(text, binary, subs, where):
    errs = []
    code = re.findall(r"```.*?```", text, re.S) + re.findall(r"`[^`\n]+`", re.sub(r"```.*?```", "", text, flags=re.S))
    for m in re.finditer(r"scriptvox ([a-z][a-z-]*)([^`\n]*)", "\n".join(code)):
        sub, rest = m.group(1), m.group(2)
        if sub not in subs:
            errs.append(f"{where}: unknown command `scriptvox {sub}`")
            continue
        for f in re.findall(r"(?<![\w-])--[a-z][a-z0-9-]*", rest):
            if f not in flags(binary, sub):
                errs.append(f"{where}: `scriptvox {sub}` has no flag {f}")
    return errs


def selftest():
    a = "the quick brown fox jumps over the lazy dog today"
    assert grams(a) & grams("so " + a), "a shared run of 8 words is found"
    assert not grams("the quick brown fox jumps over a lazy dog today") & grams(a), "7 shared words pass"
    assert frontmatter("---\nname: x\ndescription: y\n---\nbody")["name"] == "x"
    print("skills-check selftest: ok")


def main():
    if "--selftest" in sys.argv:
        return selftest()
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=str(ROOT / "target/debug/scriptvox"))
    ap.add_argument("--repos", default=str(ROOT.parent / "repos"))
    a = ap.parse_args()
    subs = subcommands(a.bin)
    repos = pathlib.Path(a.repos)
    src, nsrc = source_grams(repos) if repos.is_dir() else (set(), 0)
    errs, nskills = [], 0
    for skill in sorted((ROOT / "plugin/skills").glob("*/SKILL.md")):
        nskills += 1
        name = skill.parent.name
        text = skill.read_text()
        fm = frontmatter(text)
        if not fm:
            errs.append(f"{name}: no frontmatter")
            continue
        if fm.get("name") != name:
            errs.append(f"{name}: frontmatter name is {fm.get('name')!r}")
        d = fm.get("description", "")
        if not d or len(d) > 1024:
            errs.append(f"{name}: description missing or over 1024 characters ({len(d)})")
        if text.count("\n") > 300:
            errs.append(f"{name}: SKILL.md has {text.count(chr(10))} lines (limit 300; move detail to references/)")
        for f in [skill, *sorted(skill.parent.glob("references/*.md"))]:
            body = f.read_text()
            where = f.relative_to(ROOT)
            errs += check_commands(body, a.bin, subs, where)
            shared = grams(body) & src
            for g in sorted(shared)[:5]:
                errs.append(f"{where}: shares 8 words with a source skill: \"{g}\"")
    for e in errs:
        print("FAIL", e)
    note = f"{nsrc} source files compared" if nsrc else "originality NOT checked (../repos absent)"
    print(f"skills-check: {nskills} skills, {len(errs)} failures, {note}")
    return 1 if errs else 0


if __name__ == "__main__":
    sys.exit(main())
