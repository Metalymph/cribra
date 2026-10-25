from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "website/src/content/docs"
BASE = "https://github.com/Metalymph/cribra"

PREFIXES = ("bindings/", "crates/", "docs/", "examples/", "src/", "include/")

pattern = re.compile(
    r"(?<!\[)`((?:" + "|".join(re.escape(p) for p in PREFIXES)
    + r")[A-Za-z0-9_./-]+)`(?!\]\()"
)

changed = []
missing = set()

for page in sorted(DOCS.glob("*.md")):
    original = page.read_text()
    lines = original.splitlines(keepends=True)
    result = []
    in_fence = False

    for line in lines:
        stripped = line.lstrip()

        if stripped.startswith("```") or stripped.startswith("~~~"):
            in_fence = not in_fence
            result.append(line)
            continue

        if in_fence:
            result.append(line)
            continue

        def replace(match):
            path = match.group(1)
            target = ROOT / path

            if not target.exists():
                missing.add((page.name, path))
                return match.group(0)

            kind = "tree" if target.is_dir() else "blob"
            url = f"{BASE}/{kind}/main/{path}"

            return f"[`{path}`]({url})"

        result.append(pattern.sub(replace, line))

    updated = "".join(result)

    if updated != original:
        page.write_text(updated)
        changed.append(page.name)

print(f"Updated pages: {len(changed)}")
for name in changed:
    print(f"  {name}")

if missing:
    print("\nUnresolved source paths:")
    for page, path in sorted(missing):
        print(f"  {page}: {path}")
