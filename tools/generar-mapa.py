#!/usr/bin/env python3
"""Genera docs/mapa.md: dónde está cada archivo y para qué sirve (sin dependencias externas).

Uso (desde cualquier carpeta):  python3 tools/generar-mapa.py

El propósito de cada archivo se saca de su propio comentario de cabecera (`//!` en Rust, KDoc o
comentario inicial en Kotlin/JS, docstring o `#` en Python y shell, título y primera frase en los
Markdown de docs/). Si el archivo no tiene cabecera se marca como sin descripción y se lista al final.
Respeta .gitignore. Es idempotente: sin cambios en el código, el resultado es idéntico byte a byte.
"""
import fnmatch
import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
SALIDA = RAIZ / "docs" / "mapa.md"
SIN_DESC = "_(sin descripción en el archivo)_"
MAX_DESC = 240

EXCLUIDAS = {"target", "build", ".gradle", "node_modules", "graphify-out", "jniLibs", ".git",
             "__pycache__"}
EXCLUIDAS_RUTAS = ("web/pkg",)  # rutas desde la raíz
EXT_CODIGO = {".rs", ".kt", ".kts", ".java", ".py", ".sh", ".js", ".mjs", ".gradle", ".toml"}
NOMBRES_EXTRA = {"jitpack.yml"}

PIEZAS = [
    ("nucleo", "Núcleo (core/src)",
     "El corazón en Rust: almacenamiento, sincronización, red, cifrado, identidad, relé y la interfaz FFI que usan los SDK."),
    ("tests", "Pruebas del núcleo (core/tests)",
     "Pruebas de integración del núcleo: durabilidad, ataques, red, migración y medidas de velocidad."),
    ("android", "SDK Android",
     "La librería Kotlin para apps Android y sus piezas auxiliares (avisos, app de demostración)."),
    ("rn", "Puente React Native",
     "El módulo que expone MobileMesh a apps React Native, con su parte nativa de Android."),
    ("web", "SDK web",
     "El SDK para navegador: envoltorio JavaScript sobre el núcleo compilado a WASM."),
    ("rele", "Relé y servidor",
     "Los programas que hacen de relé y de servidor, incluida la app Android que convierte un móvil en relé."),
    ("cli", "Línea de órdenes y herramientas (core/src/bin, tools/)",
     "La CLI `mesh`, el servidor MCP y los scripts de operación del repositorio."),
    ("ejemplos", "Ejemplos",
     "Apps y ayudantes de ejemplo que muestran cómo usar MobileMesh."),
    ("docs", "Documentación",
     "Guías, protocolo y decisiones de diseño en Markdown."),
    ("build", "Scripts de compilación",
     "Configuración de Gradle, Cargo y publicación que compila y empaqueta cada pieza."),
]


# ---------------------------------------------------------------- .gitignore

def leer_gitignore():
    reglas = []
    ruta = RAIZ / ".gitignore"
    if not ruta.exists():
        print("AVISO: no hay .gitignore; solo se aplican las exclusiones fijas.")
        return reglas
    for l in ruta.read_text(encoding="utf-8").splitlines():
        l = l.strip()
        if not l or l.startswith("#") or l.startswith("!"):
            continue
        solo_dir = l.endswith("/")
        l = l.rstrip("/")
        ancla = "/" in l
        reglas.append((l.lstrip("/"), solo_dir, ancla))
    return reglas


def ignorado(rel, reglas):
    partes = rel.split("/")
    for pat, solo_dir, ancla in reglas:
        n = len(partes) if not solo_dir else len(partes) - 1  # un archivo no casa con regla de carpeta
        if ancla:
            for i in range(1, n + 1):
                if fnmatch.fnmatchcase("/".join(partes[:i]), pat):
                    return True
        else:
            for i in range(n):
                if fnmatch.fnmatchcase(partes[i], pat):
                    return True
            if not solo_dir and fnmatch.fnmatchcase(partes[-1], pat):
                return True
    return False


def es_binario(ruta):
    try:
        return b"\0" in ruta.read_bytes()[:8192]
    except OSError:
        return True


def archivos(reglas):
    res = []
    for p in sorted(RAIZ.rglob("*")):
        if not p.is_file():
            continue
        rel = p.relative_to(RAIZ).as_posix()
        partes = rel.split("/")
        if any(x in EXCLUIDAS for x in partes[:-1]):
            continue
        if any(rel.startswith(r + "/") for r in EXCLUIDAS_RUTAS):
            continue
        if ignorado(rel, reglas):
            continue
        en_docs = partes[0] == "docs" and p.suffix == ".md"
        if not (en_docs or p.suffix in EXT_CODIGO or p.name in NOMBRES_EXTRA):
            continue
        if rel == "docs/mapa.md" or es_binario(p):
            continue
        res.append(rel)
    return res


# ---------------------------------------------------------------- cabeceras

def primera_frase(texto):
    texto = " ".join(texto.split())
    m = re.search(r"(?<=[.!?])\s+(?=[A-ZÁÉÍÓÚÑ¿¡`])", texto)
    f = texto[: m.start()] if m else texto
    if len(f) > MAX_DESC:
        f = f[: MAX_DESC - 1].rstrip() + "…"
    return f


def parrafo(lineas):
    """Primer párrafo (hasta línea en blanco) de una lista de líneas ya sin marcas."""
    out = []
    for l in lineas:
        if not l.strip():
            if out:
                break
            continue
        out.append(l.strip())
    return " ".join(out)


RUIDO = re.compile(r"^(copyright|spdx|licen[cs]e|-\*-|eslint|@ts-|prettier|noinspection|vim?:)", re.I)


def descripcion(lineas):
    if not lineas:
        return ""
    # Descarta un primer párrafo que sea solo ruido (licencia, directivas de editor).
    txt = parrafo(lineas)
    if RUIDO.match(txt):
        return ""
    return primera_frase(txt)


def cab_barras(lineas, prefijo):
    """Comentario de línea inicial (`//!`, `//`, `#`) saltando shebang y líneas vacías."""
    i = 0
    if lineas and lineas[0].startswith("#!"):
        i = 1
    while i < len(lineas) and not lineas[i].strip():
        i += 1
    out = []
    while i < len(lineas):
        l = lineas[i].strip()
        if l.startswith(prefijo) and not (prefijo == "//" and l.startswith(("///", "//!"))):
            out.append(l[len(prefijo):].lstrip(" ") if not l.startswith(prefijo + prefijo[-1] * 2) else "")
        elif l.startswith("#") and prefijo == "#":
            out.append(l.lstrip("#").strip())
        else:
            break
        i += 1
    return out


def cab_bloque(lineas, permitir_codigo_previo, stem=""):
    """Primer `/** ... */` o `/* ... */` del archivo (tras package/import/@file si se permite)."""
    i = 0
    while i < len(lineas):
        l = lineas[i].strip()
        if not l or l.startswith("#!"):
            i += 1
        elif permitir_codigo_previo and re.match(r"(package|import|@file|@Suppress)\b", l):
            i += 1
        elif l.startswith("'use strict'") or l.startswith('"use strict"'):
            i += 1
        else:
            break
    if i >= len(lineas) or not lineas[i].strip().startswith("/*"):
        return None
    ini = i
    out = []
    while i < len(lineas):
        l = lineas[i].strip()
        fin = "*/" in l
        l = re.sub(r"^/\*+", "", l)
        l = re.sub(r"\*+/.*$", "", l)
        l = re.sub(r"^\*+ ?", "", l)
        out.append(l.strip())
        if fin:
            break
        i += 1
    if permitir_codigo_previo and any(re.match(r"(package|import)\b", x) for x in lineas[:ini]):
        # KDoc de una declaración, no del archivo: solo vale si va tras línea en blanco o si la
        # declaración que documenta es la que da nombre al archivo.
        sig = lineas[i + 1].strip() if i + 1 < len(lineas) else ""
        if sig and not re.search(r"\b" + re.escape(stem) + r"\b", sig):
            return None
    return out


def cab_rust(lineas):
    out = []
    for l in lineas:
        s = l.strip()
        if s.startswith("//!"):
            out.append(s[3:].strip())
        elif not s or s.startswith("#!") and not s.startswith("#![") or (not out and s.startswith("//") and not s.startswith("///")):
            if out and not s:
                out.append("")
            continue
        else:
            break
    return out


def cab_python(lineas):
    i = 0
    while i < len(lineas) and (not lineas[i].strip() or lineas[i].startswith("#")):
        # los # iniciales cuentan como cabecera si no hay docstring
        i += 1
    if i < len(lineas):
        m = re.match(r'\s*[rRuU]?("""|\'\'\')(.*)', lineas[i])
        if m:
            q, resto = m.group(1), m.group(2)
            out = []
            if q in resto:
                return [resto.split(q)[0]]
            out.append(resto)
            for l in lineas[i + 1:]:
                if q in l:
                    out.append(l.split(q)[0])
                    break
                out.append(l)
            return out
    return cab_barras(lineas, "#")


def cab_markdown(lineas):
    titulo, cuerpo = None, []
    en_codigo = False
    for l in lineas:
        s = l.strip()
        if s.startswith("```"):
            en_codigo = not en_codigo
            continue
        if en_codigo:
            continue
        if titulo is None:
            if s.startswith("#"):
                titulo = s.lstrip("#").strip()
            continue
        if s.startswith("#"):
            if cuerpo:
                break
            continue
        if not s:
            if cuerpo:
                break
            continue
        if s.startswith(("|", "---", "<")):
            continue
        cuerpo.append(re.sub(r"[*_`]|\[([^\]]*)\]\([^)]*\)", lambda m: m.group(1) or "", s))
    if titulo is None:
        return ""
    frase = primera_frase(" ".join(cuerpo)) if cuerpo else ""
    return f"{titulo}: {frase}" if frase else titulo


def proposito(rel):
    ruta = RAIZ / rel
    try:
        lineas = ruta.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeDecodeError):
        return SIN_DESC, 0
    n = len(lineas)
    ext = ruta.suffix
    if ext == ".md":
        d = cab_markdown(lineas)
    elif ext == ".rs":
        d = descripcion(cab_rust(lineas))
    elif ext == ".py":
        d = descripcion(cab_python(lineas))
    elif ext in (".sh", ".toml") or ruta.name == "jitpack.yml":
        d = descripcion(cab_barras(lineas, "#"))
    else:  # .kt .kts .java .js .mjs .gradle
        bloque = cab_bloque(lineas, ext in (".kt", ".kts", ".java"), ruta.stem)
        if bloque is None:
            bloque = cab_barras(lineas, "//")
        d = descripcion(bloque)
    return (d.replace("|", "\\|") if d else SIN_DESC), n


# ---------------------------------------------------------------- piezas

def pieza(rel):
    p = rel.split("/")
    nombre = p[-1]
    if rel == "core/uniffi-bindgen.rs":
        return "build"
    if rel == "core/src/main.rs" or rel.startswith("core/src/bin/") or p[0] == "tools":
        return "cli"
    if (nombre.endswith((".gradle", ".kts", "Cargo.toml", "config.toml")) or nombre in NOMBRES_EXTRA
            or (nombre.endswith(".sh") and p[0] != "tools" and p[0] != "examples")
            or rel.startswith("android/publicar/")):
        return "build"
    if rel.startswith("core/src/"):
        return "nucleo"
    if rel.startswith("core/tests/"):
        return "tests"
    if rel.startswith("core/js/") or p[0] == "web":
        return "web"
    if p[0] == "react-native" or rel == "react-native.config.js":
        return "rn"
    if p[0] == "servidor" or rel.startswith("android/rele-movil/"):
        return "rele"
    if p[0] == "android":
        return "android"
    if p[0] == "examples":
        return "ejemplos"
    if p[0] == "docs":
        return "docs"
    return None


def main():
    reglas = leer_gitignore()
    grupos = {k: [] for k, _, _ in PIEZAS}
    sin_pieza = []
    for rel in archivos(reglas):
        k = pieza(rel)
        if k is None:
            sin_pieza.append(rel)
            continue
        d, n = proposito(rel)
        grupos[k].append((rel, d, n))
    if sin_pieza:
        print("AVISO: archivos sin pieza asignada (no aparecen en el mapa): " + ", ".join(sin_pieza))

    total = sum(len(v) for v in grupos.values())
    huecos = [(r, k) for k, v in grupos.items() for r, d, _ in v if d == SIN_DESC]
    out = [
        "<!-- GENERADO por tools/generar-mapa.py. No editar a mano. -->",
        "# Mapa del proyecto",
        "",
        "> **Archivo GENERADO** a partir del código. No se edita a mano: se regenera con",
        "> `python3 tools/generar-mapa.py`.",
        "",
        "Dónde está cada cosa y para qué sirve. El propósito sale del comentario de cabecera de cada archivo;",
        f"si no lo tiene aparece como {SIN_DESC} y se lista al final.",
        "",
        f"**Archivos mapeados:** {total} · **sin descripción:** {len(huecos)}",
        "",
    ]
    for k, titulo, frase in PIEZAS:
        out += [f"## {titulo}", "", frase, ""]
        if not grupos[k]:
            out += ["_(sin archivos)_", ""]
            continue
        out += ["| Archivo | Para qué sirve | Líneas |", "|---|---|---:|"]
        for rel, d, n in sorted(grupos[k]):
            out.append(f"| `{rel}` | {d} | {n} |")
        out.append("")
    out += ["## Archivos sin descripción", "",
            "Deuda de documentación: ningún comentario de cabecera que explique su propósito.", ""]
    if huecos:
        nombres = {k: t for k, t, _ in PIEZAS}
        out += [f"- `{r}` ({nombres[k]})" for r, k in sorted(huecos)]
    else:
        out.append("_(ninguno)_")
    out.append("")
    SALIDA.write_text("\n".join(out), encoding="utf-8")
    print(f"docs/mapa.md: {total} archivos, {len(huecos)} sin descripción")


if __name__ == "__main__":
    sys.exit(main())
