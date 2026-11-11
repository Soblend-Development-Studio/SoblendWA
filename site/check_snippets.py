# Compiles every Rust block of site/pages against this checkout and checks that JS and Python blocks parse
# and only call methods that exist.   python site/check_snippets.py
import json
import pathlib
import re
import subprocess
import sys
import tempfile

project = pathlib.Path(__file__).resolve().parent.parent
parts = project / "site" / "pages"
crate = project / "target" / "doc-snippets"

fences = []
for path in sorted(parts.glob("*.md")):
    page = "?"
    lines = path.read_text(encoding="utf-8").splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        if line.startswith("=== "):
            page = line[4:].split("|")[0].strip()
        stripped = line.strip()
        if stripped.startswith("```"):
            info = stripped[3:].strip().split(" ", 1)
            lang = info[0] if info[0] else "text"
            label = info[1] if len(info) > 1 else ""
            body = []
            index += 1
            while not lines[index].strip().startswith("```"):
                body.append(lines[index])
                index += 1
            fences.append((page, lang, label, "\n".join(body), path.name, index))
        index += 1

print("bloques:", len(fences))
counts = {}
for fence in fences:
    counts[fence[1]] = counts.get(fence[1], 0) + 1
print(counts)

problems = []

rust = [f for f in fences if f[1] == "rust"]
src = crate / "src"
src.mkdir(parents=True, exist_ok=True)
(crate / "Cargo.toml").write_text(
    f"""[package]
name = "doc-snippets"
version = "0.0.0"
edition = "2021"

[workspace]

[dependencies]
soblendwa = {{ path = "{project.as_posix()}", features = ["sqlite", "postgres", "redis", "mongo", "testing"] }}
tokio = {{ version = "1", features = ["full"] }}
async-trait = "0.1"
""",
    encoding="utf-8",
)

PRELUDE = """#![allow(unused, clippy::all)]
use std::sync::Arc;
use soblendwa::business::Product;
use soblendwa::{ChatStore, Client, Event, EventStream, FileStore, Jid, MediaKind, MediaOptions, Message, MessageEvent, NewsletterInfo, Store};
"""
modules = [PRELUDE]
names = {}
for number, (page, lang, label, code, filename, line) in enumerate(rust):
    names[number] = (page, filename, line)
    if "#[tokio::main]" in code:
        modules.append(f"mod s{number} {{\n{code}\n}}\n")
        continue
    modules.append(
        f"""async fn s{number}(
    client: Client, chat: Jid, grupo: Jid, msg: MessageEvent, event: Event, mut events: EventStream,
    store: Arc<dyn Store>, encuesta: String, mi_numero: Jid, producto: Product, chats: Arc<ChatStore>,
    otro: NewsletterInfo,
) -> soblendwa::Result<()> {{
{code}
Ok(())
}}
"""
    )
modules.append("fn main() {}\n")
(src / "main.rs").write_text("\n".join(modules), encoding="utf-8")
print("rust a compilar:", len(rust))

result = subprocess.run(
    ["cargo", "check", "--message-format=short", "--color=never"],
    cwd=crate,
    capture_output=True,
    text=True,
    encoding="utf-8",
    errors="replace",
)
main_rs = (src / "main.rs").read_text(encoding="utf-8").splitlines()
errors = [l for l in result.stderr.splitlines() if re.match(r"^src[\\/]main\.rs:\d+:\d+: error", l)]
for error in errors:
    line_number = int(re.match(r"^src[\\/]main\.rs:(\d+)", error).group(1))
    owner = None
    for offset in range(line_number - 1, -1, -1):
        match = re.match(r"^(?:async fn|mod) s(\d+)", main_rs[offset])
        if match:
            owner = int(match.group(1))
            break
    where = names.get(owner, ("?", "?", 0))
    source = main_rs[line_number - 1].strip()
    problems.append(f"RUST [{where[0]}] {error.split(': error', 1)[1].strip()} >>> {source[:110]}")
if result.returncode != 0 and not errors:
    problems.append("RUST sin compilar: " + "\n".join(result.stderr.splitlines()[-12:]))
print("rust exit:", result.returncode, "errores:", len(errors))

node_api = (project / "bindings/node/index.js").read_text(encoding="utf-8")
client_api = (project / "clients/js/index.js").read_text(encoding="utf-8")
python_api = (project / "bindings/python/python/soblendwa/__init__.py").read_text(encoding="utf-8")
node_methods = set(re.findall(r"^\s+(?:async\s+)?\*?([a-zA-Z_]\w*)\s*\(", node_api, re.M)) | {"on", "once", "off", "emit"}
client_methods = set(re.findall(r"^\s+(?:async\s+)?\*?([a-zA-Z_]\w*)\s*\(", client_api, re.M)) | {"on", "once", "off", "emit"}
python_methods = set(re.findall(r"^\s+(?:async )?def ([a-z_][a-z0-9_]*)\(", python_api, re.M))

work = pathlib.Path(tempfile.mkdtemp())
checked = 0
for number, (page, lang, label, code, filename, line) in enumerate(fences):
    if lang == "js":
        if "soblendwa-client" in code or label == "Cliente JS" or page == "cliente-js":
            known = client_methods
        else:
            known = node_methods
        for name in re.findall(r"\bbot\.([a-zA-Z]+)\(", code):
            if name not in known:
                problems.append(f"JS [{page}] {label or 'js'}: bot.{name} no existe")
        for name in re.findall(r"\bbot\.([a-zA-Z]+)\(", code):
            pass
        target = work / f"s{number}.mjs"
        body = re.sub(r"^const (.+) = require\((.+)\)$", r"const \1 = {}", code, flags=re.M)
        body = re.sub(r"^import .+$", "const connect = async () => ({})", body, flags=re.M)
        target.write_text(f"(async () => {{\n{body}\n}})()\n", encoding="utf-8")
        run = subprocess.run(["node", "--check", str(target)], capture_output=True, text=True, encoding="utf-8", errors="replace")
        if run.returncode:
            last = [l for l in run.stderr.splitlines() if "Error" in l]
            problems.append(f"JS [{page}] {label}: {last[0] if last else run.stderr[:120]}")
        checked += 1
    elif lang == "python":
        for name in re.findall(r"\bbot\.([a-z_]+)\(", code):
            if name not in python_methods:
                problems.append(f"PY [{page}] bot.{name} no existe")
        wrapped = code if "asyncio.run" in code else "async def _wrap():\n" + "".join(f"    {l}\n" for l in code.splitlines())
        try:
            compile(wrapped, f"{page}", "exec")
        except SyntaxError as err:
            problems.append(f"PY [{page}] {err}")
        checked += 1
    elif lang == "json":
        try:
            json.loads(code)
        except ValueError as err:
            problems.append(f"JSON [{page}] {err}")
        checked += 1
print("js, python y json revisados:", checked)

for problem in problems:
    print("PROBLEMA", problem)
print("total de problemas:", len(problems))
sys.exit(1 if problems else 0)
