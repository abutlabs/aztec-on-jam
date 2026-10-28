#!/usr/bin/env python3
"""The private review copy of docs/aztec/index.html: the same page, its stylesheet inlined (the
artifact can't load ../site.css) and its theme tokens answering the viewer's light/dark switch.
   scripts/artifact-from-page.py <out.html>"""
import re, sys
root = __file__.rsplit("/scripts/", 1)[0]
page = open(f"{root}/docs/aztec/index.html", encoding="utf-8").read()
css = open(f"{root}/docs/site.css", encoding="utf-8").read()
# the dark tokens: guarded for an explicit light choice, and repeated for an explicit dark one
m = re.search(r"@media \(prefers-color-scheme: dark\) \{ :root \{(.*?)\} \}", css, re.S)
dark = m.group(1)
css = css.replace(m.group(0), "@media (prefers-color-scheme: dark) { :root:not([data-theme=\"light\"]) {%s} }\n:root[data-theme=\"dark\"] {%s}" % (dark, dark))
css = css.replace("@media (prefers-color-scheme: dark) { html { color-scheme: dark; } }", "")
head = page[page.index("<title>"):page.index("</head>")]
head = head.replace('<link rel="stylesheet" href="../site.css">', "<style>\n" + css + "\n</style>")
body = page[page.index("<body>") + len("<body>"):page.index("</body>")]
body = re.sub(r'href="\.\./([^"]*)"', r'href="https://abutlabs.github.io/aztec-on-jam/\1"', body)
open(sys.argv[1], "w", encoding="utf-8").write(head + body)
print("artifact written")
