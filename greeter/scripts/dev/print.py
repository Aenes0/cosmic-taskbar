#!/usr/bin/env python3
"""Print só da tela do compositor aninhado, em target/prints/NOME.png. Uso: print.py NOME

Usa grim apontado para o socket do cosmic-comp aninhado (lido de target/dev/nested.display,
gravado por nested-*.sh): nada da sessão real entra na imagem.
"""
import os
import re
import subprocess
import sys

if len(sys.argv) != 2 or not re.fullmatch(r"[A-Za-z0-9._-]+", sys.argv[1]):
    sys.exit("uso: print.py NOME  (letras, dígitos, . _ -)")

repo = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
display_file = os.path.join(repo, "target", "dev", "nested.display")
try:
    with open(display_file) as f:
        display = f.read().strip()
except OSError:
    sys.exit(f"{display_file} não existe: o compositor aninhado está rodando? (nested-*.sh)")
if not display:
    sys.exit(f"{display_file} está vazio")

outdir = os.path.join(repo, "target", "prints")
os.makedirs(outdir, exist_ok=True)
out = os.path.join(outdir, sys.argv[1] + ".png")
env = dict(os.environ, WAYLAND_DISPLAY=display)
res = subprocess.run(["grim", out], env=env, capture_output=True, text=True)
if res.returncode != 0 or not os.path.exists(out):
    sys.exit(f"grim falhou (código {res.returncode}): {res.stderr.strip()}")
print(out)
