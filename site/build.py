# Builds docs/index.html from site/pages/*.md.
#   python site/build.py            write the page
#   python site/build.py --check    fail if docs/index.html is stale or something is broken (CI)
#   python site/build.py --missing  list public functions with no entry in the reference
import html
import pathlib
import re
import sys
import unicodedata

root = pathlib.Path(__file__).resolve().parent.parent
parts_dir = root / "site" / "pages"
output = root / "docs" / "index.html"
checking = "--check" in sys.argv

VERSION = re.search(r'(?m)^version = "([^"]+)"', (root / "Cargo.toml").read_text(encoding="utf-8")).group(1)
MINOR = ".".join(VERSION.split(".")[:2])

# A version written by hand goes stale with the next release. Pages say {{version}} or {{minor}}; an older
# version is only allowed on a line that says it is older ("anterior").
VERSION_MENTION = re.compile(
    r'(?:(?:SoblendWA|soblendwa(?:-node|-client)?|versión|version)\s*(?:=\s*\{\s*version\s*=\s*|=\s*|==\s*|"?:\s*)?"?'
    r'|\b[Ee]n |\|\s*)(\d+\.\d{1,2}(?:\.\d+)?)(?=[\s"|,)]|\.\s|\.$|$)'
)


def stale_versions(path, text):
    found = []
    for number, line in enumerate(text.splitlines(), 1):
        if "anterior" in line:
            continue
        for match in VERSION_MENTION.finditer(line):
            if match.group(1) not in (VERSION, MINOR):
                found.append(f"{path.name}:{number}: {match.group(0)!r} (la versión es {VERSION})")
    return found


def expand(text):
    return text.replace("{{version}}", VERSION).replace("{{minor}}", MINOR)


def scan_api():
    table = {}
    pattern = re.compile(
        r"(?m)^(\s*)(impl(?:<[^>]*>)?\s+[^\{]+?\{)|^\s*pub (const )?(async )?fn ([a-z_0-9]+)(<[^\(]*?>)?\((.*?)\)(?:\s*->\s*([^\{]+?))?\s*(?:where[^\{]*)?\{",
        re.S,
    )
    for path in sorted((root / "src").rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        cut = text.find("#[cfg(test)]\nmod tests")
        if cut != -1:
            text = text[:cut]
        owner = "crate"
        for match in pattern.finditer(text):
            if match.group(2):
                raw = re.sub(r"\s+", " ", match.group(2)[:-1]).strip()
                raw = re.sub(r"^impl(<[^>]*>)?\s*", "", raw)
                owner = raw.split(" for ")[-1].strip() if " for " in raw else raw
                continue
            if match.group(1) is None and match.group(5) is None:
                continue
            name = match.group(5)
            args = re.sub(r"\s+", " ", match.group(7)).strip().rstrip(",")
            args = re.sub(r"\bmut (self|[a-z_]+:)", r"\1", args)
            ret = re.sub(r"\s+", " ", match.group(8) or "").strip()
            generics = re.sub(r"\s+", " ", match.group(6) or "")
            is_free = not match.group(0).lstrip().startswith("pub") or owner == "crate"
            file_owner = owner
            indent_free = text.rfind("\n", 0, match.start()) + 1
            if text[indent_free:match.start() + 1].strip() == "" and not match.group(0).startswith((" ", "\t", "\n")):
                pass
            line_start = text.rfind("\n", 0, match.end())
            table.setdefault((file_owner, name), {
                "async": bool(match.group(4)),
                "generics": generics,
                "args": args,
                "ret": ret,
                "file": path.relative_to(root).as_posix(),
            })
    return table


API = scan_api()
documented = set()
errors = []


def esc(text):
    return html.escape(text, quote=False)


def slug(text):
    text = unicodedata.normalize("NFD", text)
    text = "".join(c for c in text if unicodedata.category(c) != "Mn").lower()
    text = re.sub(r"`", "", text)
    text = re.sub(r"[^a-z0-9]+", "-", text).strip("-")
    return text[:60].strip("-")


PILLS = {
    "{exp}": '<span class="pill pill-exp">Experimental</span>',
    "{ok}": '<span class="pill pill-ok">Probado</span>',
    "{no}": '<span class="pill pill-no">No incluido</span>',
}


def inline(text):
    out = []
    for index, piece in enumerate(re.split(r"(`[^`]*`)", text)):
        if index % 2:
            out.append(f"<code>{esc(piece[1:-1])}</code>")
            continue
        piece = esc(piece)
        piece = re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", lambda m: link(m.group(1), m.group(2)), piece)
        piece = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", piece)
        piece = re.sub(r"(?<![\w*])\*([^*\s][^*]*)\*(?![\w*])", r"<em>\1</em>", piece)
        for key, value in PILLS.items():
            piece = piece.replace(key, value)
        out.append(piece)
    return "".join(out)


links_seen = []


def link(label, href):
    if href.startswith("#"):
        links_seen.append(href[1:])
    return f'<a href="{href}">{label}</a>'


def fix_code_links(text):
    return re.sub(r"\[<code>([^<]*)</code>\]\(([^)\s]+)\)", lambda m: link(f"<code>{m.group(1)}</code>", m.group(2)), text)


def cells(row):
    row = row.strip()[1:]
    if row.endswith("|"):
        row = row[:-1]
    items, current, in_code = [], "", False
    for char in row:
        if char == "`":
            in_code = not in_code
        if char == "|" and not in_code:
            items.append(current.strip())
            current = ""
        else:
            current += char
    items.append(current.strip())
    return items


def render_table(rows):
    head = cells(rows[0])
    body = [cells(r) for r in rows[2:]]
    out = ['<div class="table-wrap"><table><thead><tr>']
    out += [f"<th>{render_inline(c)}</th>" for c in head]
    out.append("</tr></thead><tbody>")
    for row in body:
        out.append("<tr>" + "".join(f"<td>{render_inline(c)}</td>" for c in row) + "</tr>")
    out.append("</tbody></table></div>")
    return "".join(out)


def render_inline(text):
    return fix_code_links(inline(text))


def code_block(lang, label, code):
    lang = lang or "text"
    head = f'<div class="code-label">{esc(label)}</div>' if label else ""
    return f'<div class="code-block">{head}<pre><code class="language-{lang}">{esc(code)}</code></pre></div>'


tab_counter = [0]


def render_tabs(fences):
    tab_counter[0] += 1
    group = tab_counter[0]
    buttons, panels = [], []
    for index, (lang, label, code) in enumerate(fences):
        key = slug(label) or lang
        selected = index == 0
        panel = f"tab-{group}-{index}"
        buttons.append(
            f'<button type="button" class="tab-button" role="tab" data-lang="{key}" aria-controls="{panel}" '
            f'aria-selected="{"true" if selected else "false"}" tabindex="{0 if selected else -1}">{esc(label)}</button>'
        )
        hidden = "" if selected else " hidden"
        panels.append(f'<div class="tab-panel" id="{panel}" role="tabpanel"{hidden}>{code_block(lang, "", code)}</div>')
    return f'<div class="tabs"><div class="tab-list" role="tablist">{"".join(buttons)}</div>{"".join(panels)}</div>'


def render_api_item(owner, name, description, page_id):
    entry = API.get((owner, name))
    if not entry:
        errors.append(f"sin firma: {owner}::{name}")
        return ""
    documented.add((owner, name))
    aliases = {"BatchedStore<Mongo>": "MongoStore", "BatchedStore<Postgres>": "PostgresStore", "BatchedStore<Redis>": "RedisStore", "BatchedStore<Sqlite>": "SqliteStore", "BatchedStore<B>": "BatchedStore", "crate::client::Stats": "Stats"}
    label = aliases.get(owner, owner)
    item_id = f"api-{slug(label)}-{name}".replace("_", "-")
    args = entry["args"]
    ret = f' -> {entry["ret"]}' if entry["ret"] else ""
    pill = '<span class="pill pill-async">async</span>' if entry["async"] else ""
    signature = f'<span class="api-name">{esc(name)}</span>{esc(entry["generics"])}({esc(args)}){esc(ret)}'
    display = f"{label}::{name}" if not args.startswith(("&self", "self")) else f"{label}.{name}"
    if label == "crate":
        display = name
    return (
        f'<div class="api" id="{item_id}" data-name="{esc(display)}"><div class="api-sig">{pill}<code>{signature}</code></div>'
        f"<p>{render_inline(description)}</p></div>"
    )


def render_manual_item(name, signature, description, page_id, prefix):
    item_id = f"{prefix}-{slug(name)}"
    return (
        f'<div class="api" id="{item_id}" data-name="{esc(name)}"><div class="api-sig"><code>{esc(signature)}</code></div>'
        f"<p>{render_inline(description)}</p></div>"
    )


def render_blocks(lines, page_id, used):
    out = []
    index = 0
    paragraph = []

    def flush():
        if paragraph:
            out.append(f"<p>{render_inline(' '.join(paragraph))}</p>")
            paragraph.clear()

    while index < len(lines):
        line = lines[index]
        stripped = line.strip()
        if not stripped:
            flush()
            index += 1
            continue
        if stripped.startswith("```"):
            flush()
            info = stripped[3:].strip().split(" ", 1)
            lang = info[0] if info and info[0] else "text"
            label = info[1] if len(info) > 1 else ""
            index += 1
            body = []
            while not lines[index].strip().startswith("```"):
                body.append(lines[index])
                index += 1
            index += 1
            out.append(code_block(lang, label, "\n".join(body)))
            continue
        if stripped.startswith(":::"):
            flush()
            head = stripped[3:].strip().split(" ", 1)
            kind = head[0]
            title = head[1] if len(head) > 1 else ""
            index += 1
            body = []
            depth = 0
            in_fence = False
            while True:
                current = lines[index]
                if current.strip().startswith("```"):
                    in_fence = not in_fence
                if not in_fence and current.strip() == ":::":
                    break
                body.append(current)
                index += 1
            index += 1
            if kind == "tabs":
                fences = []
                position = 0
                while position < len(body):
                    text = body[position].strip()
                    if text.startswith("```"):
                        info = text[3:].strip().split(" ", 1)
                        position += 1
                        code = []
                        while not body[position].strip().startswith("```"):
                            code.append(body[position])
                            position += 1
                        fences.append((info[0], info[1] if len(info) > 1 else info[0], "\n".join(code)))
                    position += 1
                out.append(render_tabs(fences))
            elif kind in ("note", "tip", "warn", "danger"):
                names = {"note": "Nota", "tip": "Consejo", "warn": "Cuidado", "danger": "Importante"}
                inner = render_blocks(body, page_id, used)
                out.append(
                    f'<aside class="callout callout-{kind}"><p class="callout-title">{esc(title or names[kind])}</p>{inner}</aside>'
                )
            elif kind == "steps":
                items = [b.strip()[2:].strip() for b in body if b.strip().startswith("- ")]
                out.append('<ol class="steps">' + "".join(f"<li>{render_inline(i)}</li>" for i in items) + "</ol>")
            elif kind == "cards":
                cards = []
                for entry in body:
                    if "::" not in entry:
                        continue
                    left, right = entry.split("::", 1)
                    match = re.match(r"\s*\[([^\]]+)\]\(([^)]+)\)", left)
                    if match.group(2).startswith("#"):
                        links_seen.append(match.group(2)[1:])
                    cards.append(
                        f'<a class="card" href="{match.group(2)}"><span class="card-title">{esc(match.group(1))}</span>'
                        f'<span class="card-text">{render_inline(right.strip())}</span></a>'
                    )
                out.append(f'<div class="card-grid">{"".join(cards)}</div>')
            elif kind == "stats":
                stats = []
                for entry in body:
                    if "::" not in entry:
                        continue
                    value, label = entry.split("::", 1)
                    stats.append(
                        f'<div class="stat"><span class="stat-value">{esc(value.strip())}</span>'
                        f'<span class="stat-label">{render_inline(label.strip())}</span></div>'
                    )
                out.append(f'<div class="stat-row">{"".join(stats)}</div>')
            elif kind == "raw":
                out.append("\n".join(body))
            else:
                errors.append(f"bloque desconocido {kind}")
            continue
        if stripped.startswith(("@fn ", "@m ")):
            flush()
            items = []
            while index < len(lines) and lines[index].strip().startswith(("@fn ", "@m ")):
                text = lines[index].strip()
                if text.startswith("@fn "):
                    head, description = text[4:].split(" :: ", 1)
                    owner, name = head.rsplit(" ", 1)
                    items.append(render_api_item(owner, name, description, page_id))
                else:
                    head, signature, description = text[3:].split(" :: ", 2)
                    items.append(render_manual_item(head, signature, description, page_id, page_id))
                index += 1
            out.append(f'<div class="api-list">{"".join(items)}</div>')
            continue
        heading = re.match(r"^(#{2,4}) (.+?)(?:\s*\{#([a-z0-9-]+)\})?$", stripped)
        if heading:
            flush()
            level = len(heading.group(1))
            text = heading.group(2)
            base = heading.group(3) or f"{page_id}-{slug(text)}"
            anchor = base
            counter = 2
            while anchor in used:
                anchor = f"{base}-{counter}"
                counter += 1
            used.add(anchor)
            if level == 4:
                out.append(f"<h4>{render_inline(text)}</h4>")
            else:
                out.append(
                    f'<h{level} id="{anchor}">{render_inline(text)}'
                    f'<a class="heading-anchor" href="#{anchor}" aria-label="Enlace a esta sección">#</a></h{level}>'
                )
            index += 1
            continue
        if stripped.startswith("|"):
            flush()
            rows = []
            while index < len(lines) and lines[index].strip().startswith("|"):
                rows.append(lines[index])
                index += 1
            out.append(render_table(rows))
            continue
        if re.match(r"^(- |\d+\. )", stripped):
            flush()
            ordered = bool(re.match(r"^\d+\. ", stripped))
            items = []
            while index < len(lines):
                current = lines[index]
                text = current.strip()
                if re.match(r"^(- |\d+\. )", text):
                    items.append(re.sub(r"^(- |\d+\. )", "", text))
                elif text and current.startswith("  ") and items:
                    items[-1] += " " + text
                else:
                    break
                index += 1
            tag = "ol" if ordered else "ul"
            out.append(f"<{tag}>" + "".join(f"<li>{render_inline(i)}</li>" for i in items) + f"</{tag}>")
            continue
        paragraph.append(stripped)
        index += 1
    flush()
    return "\n".join(out)


pages = []
current = None
stale = []
for path in sorted(parts_dir.glob("*.md")):
    source = path.read_text(encoding="utf-8")
    stale += stale_versions(path, source)
    # @include brings in a file from the repo as is, so the changelog lives in one place only.
    source = re.sub(
        r"(?m)^@include (\S+)$",
        lambda m: (root / m.group(1)).read_text(encoding="utf-8").split("\n", 1)[1],
        source,
    )
    for line in expand(source).splitlines():
        if line.startswith("=== "):
            fields = [f.strip() for f in line[4:].split("|")]
            current = {"id": fields[0], "title": fields[1], "group": fields[2], "flags": fields[3] if len(fields) > 3 else "", "lines": []}
            pages.append(current)
        elif current is not None:
            current["lines"].append(line)

used_ids = {p["id"] for p in pages}
page_html = []
groups = []
for page in pages:
    lines = page["lines"]
    lead = ""
    eyebrow = page["group"]
    while lines and not lines[0].strip():
        lines.pop(0)
    if lines and lines[0].startswith("lead: "):
        lead = lines.pop(0)[6:].strip()
    body = render_blocks(lines, page["id"], used_ids)
    wide = ' data-wide="true"' if "wide" in page["flags"] else ""
    if "bare" in page["flags"]:
        head = ""
    else:
        lead_html = f'<p class="lead">{render_inline(lead)}</p>' if lead else ""
        head = f'<header class="page-head"><p class="eyebrow">{esc(eyebrow)}</p><h1>{esc(page["title"])}</h1>{lead_html}</header>'
    page_html.append(
        f'<article class="page" id="{page["id"]}" data-title="{esc(page["title"])}" data-group="{esc(page["group"])}"{wide}>\n{head}\n{body}\n</article>'
    )
    if page["group"] not in [g[0] for g in groups]:
        groups.append((page["group"], []))
    next(g for g in groups if g[0] == page["group"])[1].append(page)

nav = []
for name, members in groups:
    links = "".join(f'<li><a class="nav-link" href="#{m["id"]}">{esc(m["title"])}</a></li>' for m in members)
    nav.append(f'<div class="nav-group"><p class="nav-title">{esc(name)}</p><ul class="nav-list">{links}</ul></div>')

LOGO = (
    '<svg viewBox="0 0 64 64" aria-hidden="true">'
    '<path fill="#111114" d="M18 3H46A15 15 0 0 1 61 18V42A15 15 0 0 1 46 57H21L8 63L11 54.4A15 15 0 0 1 3 42V18A15 15 0 0 1 18 3Z"/>'
    '<path fill="none" stroke="#ff2a3a" stroke-opacity="0.55" stroke-width="1.6" d="M18 3.8H46A14.2 14.2 0 0 1 60.2 18V42A14.2 14.2 0 0 1 46 56.2H20.8L9.6 61.4L12 54A14.2 14.2 0 0 1 3.8 42V18A14.2 14.2 0 0 1 18 3.8Z"/>'
    '<path fill="none" stroke="#f0182e" stroke-width="7.5" transform="translate(32 31) skewX(-12) translate(-32 -31)" d="M44 16H27A7.5 7.5 0 0 0 27 31H37A7.5 7.5 0 0 1 37 46H20"/>'
    "</svg>"
)
SEARCH = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="11" cy="11" r="7"></circle><path d="m20 20-3.5-3.5"></path></svg>'
THEME = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="4"></circle><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"></path></svg>'
MENU = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M4 7h16M4 12h16M4 17h16"></path></svg>'

document = f"""<!DOCTYPE html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Documentación de SoblendWA</title>
<meta name="description" content="Documentación completa de SoblendWA, el cliente de WhatsApp Web multi-dispositivo en Rust para bots en Rust, Node.js, TypeScript, Python y cualquier lenguaje. Hecho por Soblend Development Studio.">
<link rel="icon" href="favicon.svg" type="image/svg+xml">
<meta property="og:title" content="SoblendWA">
<meta property="og:description" content="WhatsApp Web en Rust, para bots en Rust, Node.js, Python o cualquier lenguaje.">
<meta property="og:image" content="https://raw.githubusercontent.com/Soblend-Development-Studio/SoblendWA/main/assets/social-preview.png">
<meta property="og:url" content="https://soblend-development-studio.github.io/SoblendWA/">
<meta name="twitter:card" content="summary_large_image">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Instrument+Sans:wght@400..700&family=JetBrains+Mono:wght@400;600&family=Newsreader:ital,opsz,wght@0,6..72,300..700;1,6..72,300..700&display=swap">
<link rel="stylesheet" href="styles.css">
<script>
document.documentElement.classList.add('js');
try {{
  var savedTheme = localStorage.getItem('soblendwa-theme');
  if (savedTheme === 'dark' || savedTheme === 'light') document.documentElement.dataset.theme = savedTheme;
}} catch (error) {{}}
</script>
</head>
<body>
<a class="skip-link" href="#contenido">Saltar al contenido</a>
<div class="topbar">
  <button type="button" class="icon-button menu-button" aria-label="Abrir el menú">{MENU}</button>
  <a class="brand" href="#inicio">{LOGO}<span class="brand-name">SoblendWA</span></a>
  <div class="topbar-actions">
    <button type="button" class="icon-button search-trigger-mini search-trigger" aria-label="Buscar en la documentación">{SEARCH}</button>
    <button type="button" class="icon-button theme-toggle" aria-label="Cambiar de tema">{THEME}</button>
  </div>
</div>
<div class="sidebar-scrim"></div>
<div class="layout">
  <aside class="sidebar" aria-label="Navegación de la documentación">
    <a class="brand" href="#inicio">{LOGO}<span class="brand-name">SoblendWA</span><span class="brand-version">{VERSION}</span></a>
    <button type="button" class="search-trigger" aria-label="Buscar en la documentación">{SEARCH}<span>Buscar</span><kbd>Ctrl K</kbd></button>
    <nav class="nav">
{"".join(nav)}
    </nav>
    <div class="sidebar-foot">
      <a href="https://github.com/Soblend-Development-Studio/SoblendWA">GitHub</a>
      <button type="button" class="icon-button theme-toggle" aria-label="Cambiar de tema">{THEME}</button>
    </div>
  </aside>
  <div class="main" id="contenido">
    <main class="content">
{chr(10).join(page_html)}
      <nav class="pager" aria-label="Páginas siguiente y anterior"></nav>
      <p class="site-foot">SoblendWA {VERSION} · MIT · Soblend Development Studio. Proyecto independiente, sin relación con WhatsApp ni Meta.</p>
    </main>
    <aside class="toc" aria-label="Contenido de esta página"></aside>
  </div>
</div>
<div class="search-layer" role="dialog" aria-modal="true" aria-label="Buscar en la documentación">
  <div class="search-panel">
    <div class="search-box">{SEARCH}<input id="search-input" type="search" placeholder="Busca un método, un evento o un tema" autocomplete="off" spellcheck="false" aria-label="Buscar"><kbd>Esc</kbd></div>
    <ul class="search-results" role="listbox" aria-label="Resultados"></ul>
  </div>
</div>
<script src="https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.9.0/highlight.min.js" defer></script>
<script src="app.js" defer></script>
</body>
</html>
"""

if not checking:
    output.parent.mkdir(exist_ok=True)
    output.write_text(document, encoding="utf-8", newline="\n")
    (root / "docs" / ".nojekyll").write_text("", encoding="utf-8")

public_owners = ["Client", "ClientBuilder", "Message", "ChatStore", "Jid", "FakeWhatsApp", "GroupInfo", "MessageEvent", "MessageInfo", "Proxy", "Interactive", "Section", "FileStore", "MemoryStore", "Contact", "StoredMessage", "Stats", "SendTimings", "Node", "ZapoMongo", "Error", "Event", "KeyPair", "Creds", "SignedPreKey", "Value", "PrivacySetting", "PrivacyValue", "KeyKind", "BatchedStore<Mongo>", "BatchedStore<Postgres>", "BatchedStore<Redis>", "BatchedStore<Sqlite>", "BatchedStore<B>"]
missing = sorted(key for key in API if key not in documented and key[0] in public_owners)
ids = re.findall(r'\bid="([^"]+)"', document)
duplicates = sorted({i for i in ids if ids.count(i) > 1})
known = set(ids)
broken = sorted({h for h in links_seen if h not in known})
reserved = [i for i in ids if i in ("settings", "sidebar", "customize", "upgrade", "downloads") or i.startswith("comment-")]

print(f"paginas: {len(pages)}")
print(f"tamano: {len(document.encode('utf-8')) / 1024:.0f} KB")
print(f"api documentada: {len(documented)} de {len([k for k in API if k[0] in public_owners])} en los tipos publicos")
print(f"ids duplicados: {duplicates or 'ninguno'}")
print(f"enlaces rotos: {broken or 'ninguno'}")
print(f"ids reservados: {reserved or 'ninguno'}")
for problem in errors:
    print("ERROR", problem)
for problem in stale:
    print("VERSION VIEJA", problem)
if "--missing" in sys.argv:
    for owner, name in missing:
        print("SIN DOCUMENTAR", owner, name)
else:
    print(f"sin documentar: {len(missing)}")

failed = bool(errors or stale or broken or duplicates or reserved)
if checking:
    published = output.read_text(encoding="utf-8") if output.exists() else ""
    if published.replace("\r\n", "\n") != document:
        print("docs/index.html no corresponde a site/pages: corre python site/build.py")
        failed = True
sys.exit(1 if failed else 0)
