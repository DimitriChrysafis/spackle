"""Split a template into text and tag tokens: {{ name }},
{{#each items}}...{{/each}}, {{#if cond}}...{{/if}}."""
import re

TAG = re.compile(r"{{\s*(.*?)\s*}}")

def lex(text):
    toks = []
    pos = 0
    for m in TAG.finditer(text):
        if m.start() > pos:
            toks.append(("text", text[pos:m.start()]))
        toks.append(("tag", m.group(1)))
        pos = m.end()
    if pos < len(text):
        toks.append(("text", text[pos:]))
    return toks
