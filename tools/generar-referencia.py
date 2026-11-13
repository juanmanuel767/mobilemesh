#!/usr/bin/env python3
"""Genera docs/referencia-api.md a partir del código (sin dependencias externas).

Uso (desde cualquier carpeta):  python3 tools/generar-referencia.py

Lee: SDK de Android (Kotlin), SDK web (JS), React Native (JS), la CLI `mesh` (constante USO de
core/src/main.rs) y el servidor MCP (core/src/bin/mesh-mcp.rs). Si falta algún archivo lo dice y sigue.
Es idempotente: sin cambios en el código, el resultado es idéntico byte a byte.
"""
import re
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
SALIDA = RAIZ / "docs" / "referencia-api.md"
KT_DIR = RAIZ / "android/mobilemesh-sdk/src/main/java/org/mobilemesh/sdk"
KT_PRINCIPAL = "MobileMesh.kt"
# En los .kt que no son MobileMesh.kt hay objetos públicos que son piezas internas (en español, sin uso
# para el desarrollador). Solo se documentan estos; el script avisa de los demás que omite.
KT_PUBLICOS_EXTRA = {"BackgroundMode", "Permissions", "KeystoreGuard", "DEFAULT_PORT"}
RUTAS = {
    "kotlin": KT_DIR,
    "web": RAIZ / "web/mobilemesh.js",
    "rn": RAIZ / "react-native/index.js",
    "cli": RAIZ / "core/src/main.rs",
    "mcp": RAIZ / "core/src/bin/mesh-mcp.rs",
}


def leer(ruta):
    try:
        return ruta.read_text(encoding="utf-8")
    except OSError:
        print(f"AVISO: no encuentro {ruta.relative_to(RAIZ)}; esa sección queda vacía.")
        return None


def primera_frase(texto):
    texto = " ".join(texto.split())
    m = re.search(r"(?<!\.\.)(?<=[.!?])\s", texto)
    return texto[: m.start()] if m else texto


def esc(t):
    return t.replace("|", "\\|")


# ---------------------------------------------------------------- Kotlin

def quitar_cadenas(linea):
    linea = re.sub(r'"(?:\\.|[^"\\])*"', '""', linea)
    linea = re.sub(r"'(?:\\.|[^'\\])'", "''", linea)
    return re.sub(r"//.*", "", linea)


def profundidades(lineas):
    """Profundidad de llaves antes de cada línea (ignora KDoc, comentarios y cadenas)."""
    prof, d, en_doc = [], 0, False
    for l in lineas:
        prof.append(d)
        s = l.strip()
        if en_doc:
            en_doc = "*/" not in s
            continue
        if s.startswith("/*"):
            en_doc = "*/" not in s
            continue
        c = quitar_cadenas(l)
        d += c.count("{") - c.count("}")
    return prof, d


def cortar_firma(texto, es_propiedad=False):
    """Quita el cuerpo: lo que va tras `{`, tras un `=` de nivel superior o tras `get()`."""
    pr = ang = 0
    for i, ch in enumerate(texto):
        if ch == "(":
            pr += 1
        elif ch == ")":
            pr -= 1
        elif ch == "<":
            ang += 1
        elif ch == ">" and texto[i - 1] != "-":
            ang -= 1
        elif pr == 0 and ang <= 0:
            if ch == "{":
                return texto[:i]
            if ch == "=" and texto[i + 1 : i + 2] != "=" and texto[i - 1] not in "!<>=":
                return texto[:i]
            if texto.startswith(" get()", i) and es_propiedad or texto.startswith(" by ", i):
                return texto[:i]
    return texto


def juntar_firma(lineas, i):
    """Une las líneas de una declaración hasta cerrar sus paréntesis."""
    txt, bal = "", 0
    while i < len(lineas):
        c = quitar_cadenas(lineas[i])
        txt += " " + lineas[i].strip()
        bal += c.count("(") - c.count(")")
        i += 1
        if bal <= 0:
            break
    return re.sub(r"\s+", " ", txt).strip().replace("( ", "(").replace(" )", ")").replace(",)", ")")


DECL = re.compile(
    r"^(?P<mods>(?:(?:public|private|internal|protected|override|open|abstract|sealed|data|enum|const|"
    r"companion|inline|suspend|lateinit|@\w+(?:\([^)]*\))?)\s+)*)"
    r"(?P<kind>fun|val|var|class|object|interface|typealias)\b\s*(?P<rest>.*)$"
)


def parsear_kotlin(texto, nombre_archivo, es_principal):
    """Devuelve (items, obsoletos, omitidos). item = dict(owner, kind, sig, doc, static)."""
    lineas = texto.splitlines()
    prof, final = profundidades(lineas)
    if final != 0:
        print(f"AVISO: llaves desequilibradas en {nombre_archivo} (profundidad final {final}); revisa el resultado.")
    items, obsoletos, omitidos = [], 0, []
    pila = []  # dicts: owner, depth (profundidad de sus miembros), skip, enum, static
    doc, deprecado, en_doc, doc_lineas = None, False, False, []
    i = 0
    while i < len(lineas):
        s = lineas[i].strip()
        while pila and prof[i] < pila[-1]["depth"]:
            pila.pop()
        if en_doc:
            doc_lineas.append(re.sub(r"^\*+\s?", "", s.replace("*/", "")))
            if "*/" in s:
                en_doc = False
                doc = doc_lineas
            i += 1
            continue
        if s.startswith("/**"):
            en_doc, doc_lineas = True, [re.sub(r"^/\*\*\s?", "", s.replace("*/", ""))]
            if "*/" in s:
                en_doc, doc = False, doc_lineas
            i += 1
            continue
        if not s or s.startswith("//") or s.startswith("/*"):
            i += 1
            continue
        nivel = pila[-1]["depth"] if pila else 0
        if prof[i] != nivel:  # dentro del cuerpo de una función o expresión
            i += 1
            continue
        ctx = pila[-1] if pila else None
        if ctx and ctx["skip"]:
            i += 1
            continue
        if re.match(r"@Deprecated\b", s):
            deprecado = True
            i += 1
            while "(" in s and s.count("(") > s.count(")"):  # anotación en varias líneas
                s += lineas[i].strip()
                i += 1
            continue
        # anotaciones sueltas (@Volatile, @Synchronized…) que preceden a la declaración
        m = DECL.match(s)
        if not m and ctx and ctx["enum"]:
            e = re.match(r"^([A-Z][A-Z0-9_]*)\s*[,;]?\s*$", s)
            if e:
                items.append(dict(owner=ctx["owner"], kind="entry", sig=e.group(1), doc=doc, static=False))
                doc, deprecado = None, False
                i += 1
                continue
        if not m:
            if s.startswith("@"):
                i += 1
                continue
            i += 1
            doc, deprecado = None, False
            continue
        mods, kind = m.group("mods").split(), m.group("kind")
        sig_raw = juntar_firma(lineas, i)
        nxt = i + 1
        # `kind` con modificador compuesto: «enum class», «data class», «companion object»
        if kind == "class" and "enum" in mods:
            tipo = "enum class"
        elif kind == "class" and "data" in mods:
            tipo = "data class"
        elif kind == "object" and "companion" in mods:
            tipo = "companion"
        else:
            tipo = kind
        privado = bool({"private", "internal", "protected", "override"} & set(mods))
        es_deprecado = deprecado
        d_doc, doc, deprecado = doc, None, False
        # empujar contenedor si abre llaves en la declaración
        abre = quitar_cadenas(sig_raw).count("{") > quitar_cadenas(sig_raw).count("}") or (
            tipo in ("class", "enum class", "data class", "object", "companion", "interface")
            and i + 1 < len(lineas) and prof[min(i + 1, len(lineas) - 1)] > prof[i]
        )
        if tipo == "companion":
            if abre:
                pila.append(dict(owner=ctx["owner"] if ctx else "", depth=prof[i] + 1, skip=False, enum=False, static=True))
            i = nxt
            continue
        nombre = re.match(r"(?:<[^>]*>\s*)?(?:[\w.<>?]+\.)?(\w+)", m.group("rest"))
        nombre = nombre.group(1) if nombre else "?"
        if tipo in ("class", "enum class", "data class", "object", "interface"):
            if privado or es_deprecado:
                if es_deprecado and not privado:
                    obsoletos += 1
                if abre:
                    pila.append(dict(owner=nombre, depth=prof[i] + 1, skip=True, enum=False, static=False))
                i = nxt
                continue
            if not es_principal and nombre not in KT_PUBLICOS_EXTRA:
                omitidos.append(nombre)
                if abre:
                    pila.append(dict(owner=nombre, depth=prof[i] + 1, skip=True, enum=False, static=False))
                i = nxt
                continue
            cab = cortar_firma(sig_raw).strip()
            cab = re.sub(r"^(?:public\s+)", "", cab)
            props = []
            ctor = re.search(r"\b(\w+)\s+constructor\s*\(", cab)
            if tipo != "data class" and ctor and ctor.group(1) in ("private", "internal", "protected"):
                # constructor no público: se muestra solo el nombre y se extraen las propiedades públicas
                params = cab[cab.index("(", ctor.start()) + 1 : cab.rindex(")")]
                props = [p.strip() for p in re.split(r",\s*(?![^<(]*[>)])", params) if re.match(r"\s*(val|var)\s", p)]
                cab = cab[: ctor.start()].strip()
            elif tipo != "data class" and "(" in cab:
                pass  # constructor público: se deja la firma tal cual
            items.append(dict(owner=nombre, kind=tipo, sig=cab, doc=d_doc, static=False, top=True))
            for p in props:
                items.append(dict(owner=nombre, kind="val", sig=p, doc=None, static=False))
            if abre:
                pila.append(dict(owner=nombre, depth=prof[i] + 1, skip=False, enum=tipo == "enum class", static=False))
            i = nxt
            continue
        # fun / val / var / typealias / const
        if privado:
            i = nxt
            continue
        if es_deprecado:
            obsoletos += 1
            i = nxt
            continue
        sig = cortar_firma(sig_raw, tipo in ("val", "var")).strip()
        sig = re.sub(r"^@\w+\s+", "", sig)
        if tipo == "typealias":
            sig = sig_raw.strip()
        if "const" in mods:
            val = re.search(r"=\s*(\S+)", sig_raw)
            sig = cortar_firma(sig_raw, True).strip() + (f" = {val.group(1)}" if val else "")
        owner = ctx["owner"] if ctx else ""
        if not owner and not es_principal and nombre not in KT_PUBLICOS_EXTRA:
            i = nxt
            continue
        items.append(dict(owner=owner, kind=tipo, sig=sig, doc=d_doc, static=bool(ctx and ctx["static"])))
        i = nxt
    return items, obsoletos, omitidos


def doc_a_frase(doc):
    if doc is None:
        return ""
    parrafo = []
    for l in doc:
        if not l.strip():
            if parrafo:
                break
            continue
        parrafo.append(l.strip())
    return primera_frase(" ".join(parrafo))


def seccion_android():
    if not KT_DIR.is_dir():
        print(f"AVISO: no encuentro {KT_DIR.relative_to(RAIZ)}; sección de Android vacía.")
        return ["_No se encontró el código del SDK de Android._", ""], 0
    archivos = sorted(KT_DIR.glob("*.kt"), key=lambda p: (p.name != KT_PRINCIPAL, p.name))
    todos, obs, omit = [], 0, []
    for f in archivos:
        t = leer(f)
        if t is None:
            continue
        it, o, om = parsear_kotlin(t, f.name, f.name == KT_PRINCIPAL)
        todos += [dict(x, archivo=f.name) for x in it]
        obs += o
        omit += om
    # agrupar por propietario conservando el orden de aparición
    orden, grupos = [], {}
    for x in todos:
        if x["owner"] not in grupos:
            grupos[x["owner"]] = []
            orden.append(x["owner"])
        grupos[x["owner"]].append(x)
    out = [
        "Todo vive en el paquete `org.mobilemesh.sdk` (`import org.mobilemesh.sdk.MobileMesh`). "
        "Descripciones: primera frase del KDoc tal como está en el código.",
        "",
    ]
    funcs = 0
    for o in orden:
        g = grupos[o]
        cab = next((x for x in g if x.get("top")), None)
        if cab:
            out += [f"### `{o}`", "", f"`{cab['sig']}`" + (f" — {doc_a_frase(cab['doc'])}" if doc_a_frase(cab["doc"]) else ""), ""]
            resto = [x for x in g if x is not cab]
        else:
            out += ["### Declaraciones de nivel superior", ""]
            resto = g
        entradas = [x for x in resto if x["kind"] == "entry"]
        if entradas:
            out.append("Valores:")
            out.append("")
            for x in entradas:
                d = doc_a_frase(x["doc"])
                out.append(f"- `{x['sig']}`" + (f" — {d}" if d else ""))
            out.append("")
        miembros = [x for x in resto if x["kind"] != "entry"]
        if miembros:
            out += ["| Firma | Qué hace |", "|---|---|"]
            for x in miembros:
                d = doc_a_frase(x["doc"]) or "_(sin KDoc)_"
                firma = x["sig"]
                if x["static"] and o:  # función del companion: se llama como `MobileMesh.start(...)`
                    firma = re.sub(r"^(fun|val|var) ", lambda mo: f"{mo.group(1)} {o}.", firma)
                out.append(f"| `{esc(firma)}`{' (estática)' if x['static'] else ''} | {esc(d)} |")
                if x["kind"] == "fun":
                    funcs += 1
                elif x["kind"] in ("val", "var"):
                    funcs += 1
            out.append("")
    if omit:
        out += [f"_Piezas públicas internas no documentadas aquí: {', '.join(sorted(set(omit)))}._", ""]
    out += [
        f"**Alias obsoletos:** el SDK conserva {obs} declaraciones en español marcadas `@Deprecated` "
        "(`guardar`, `coleccion`, `iniciar`, `EstadoCuenta`…). Siguen compilando, pero no son la API: "
        "no las uses en código nuevo.",
        "",
    ]
    return out, funcs


# ---------------------------------------------------------------- JavaScript

def jsdoc_previo(lineas, i):
    """Texto del bloque /** … */ que termina justo en la línea anterior a i, o None."""
    j = i - 1
    if j < 0 or not lineas[j].strip().endswith("*/"):
        return None
    k = j
    while k >= 0 and "/**" not in lineas[k]:
        k -= 1
    if k < 0:
        return None
    bloque = " ".join(lineas[k : j + 1])
    bloque = re.sub(r"/\*\*|\*/", "", bloque)
    bloque = re.sub(r"\s\*\s", " ", bloque)
    return " ".join(bloque.split())


def seccion_js_web():
    t = leer(RUTAS["web"])
    if t is None:
        return ["_No se encontró `web/mobilemesh.js`._", ""], 0
    lineas = t.splitlines()
    out = ["| Método | Qué hace |", "|---|---|"]
    n = 0
    en_clase = False
    for i, l in enumerate(lineas):
        if re.match(r"^export class MobileMesh\b", l):
            en_clase = True
            continue
        if not en_clase:
            continue
        m = re.match(r"^  (static )?(async )?(\w+)\(([^)]*)\)\s*\{", l)
        if not m or m.group(3) == "constructor":
            continue
        firma = f"{'static ' if m.group(1) else ''}{'async ' if m.group(2) else ''}{m.group(3)}({m.group(4)})"
        d = primera_frase(jsdoc_previo(lineas, i) or "") or "_(sin comentario)_"
        out.append(f"| `{esc(firma)}` | {esc(d)} |")
        n += 1
    out.append("")
    return out, n


def seccion_js_rn():
    t = leer(RUTAS["rn"])
    if t is None:
        return ["_No se encontró `react-native/index.js`._", ""], 0
    lineas = t.splitlines()
    out = ["Se usa como `MobileMesh.<método>` tras `import { MobileMesh } from \"mobilemesh\"`.", "", "| Método | Qué hace |", "|---|---|"]
    n = 0
    en_obj = False
    for i, l in enumerate(lineas):
        if re.match(r"^export const MobileMesh = \{", l):
            en_obj = True
            continue
        if en_obj and l.startswith("};"):
            en_obj = False
        m = re.match(r"^  (\w+): (async )?\(([^)]*)\)\s*=>", l) if en_obj else None
        if m:
            firma = f"{m.group(1)}({m.group(3)})"
            d = primera_frase(jsdoc_previo(lineas, i) or "") or "_(sin comentario)_"
            out.append(f"| `{'async ' if m.group(2) else ''}{esc(firma)}` | {esc(d)} |")
            n += 1
    for i, l in enumerate(lineas):
        m = re.match(r"^export function (\w+)\(([^)]*)\)", l)
        if m:
            d = primera_frase(jsdoc_previo(lineas, i) or "") or "_(sin comentario)_"
            out.append(f"| `function {m.group(1)}({esc(m.group(2))})` (también como `MobileMesh.{m.group(1)}`) | {esc(d)} |")
            n += 1
    out.append("")
    return out, n


# ---------------------------------------------------------------- CLI

def seccion_cli():
    t = leer(RUTAS["cli"])
    if t is None:
        return ["_No se encontró `core/src/main.rs`._", ""], 0
    m = re.search(r'const USO: &str = "uso:\n(.*?)";', t, re.S)
    if not m:
        print("AVISO: no encuentro la constante USO en core/src/main.rs.")
        return ["_No se encontró la constante `USO`._", ""], 0
    filas = []
    for l in m.group(1).splitlines():
        mm = re.match(r"^  mesh <carpeta> (.*)$", l)
        if mm:
            # orden = verbo + argumentos (<…>, […] o a|b|c); lo que sigue es la descripción
            partes = re.match(r"^(\S+(?:\s+(?:<[^>]+>|\[[^\]]+\]|\w+(?:\|\w+)+))*)\s*(.*)$", mm.group(1).rstrip())
            filas.append([partes.group(1), partes.group(2).strip()])
        elif filas and l.strip():
            filas[-1][1] = (filas[-1][1] + " " + l.strip()).strip()
    # una orden con varias formas: la forma sin descripción hereda la de la siguiente con el mismo verbo
    for k in range(len(filas) - 1):
        if not filas[k][1] and filas[k][0].split()[0] == filas[k + 1][0].split()[0]:
            filas[k][1] = filas[k + 1][1]
    out = [
        "Se invoca como `mesh <carpeta> <orden> …` (`<carpeta>` es la del dispositivo simulado).",
        "",
        "| Orden | Qué hace |",
        "|---|---|",
    ]
    for o, d in filas:
        out.append(f"| `mesh <carpeta> {esc(o)}` | {esc(d) if d else '_(sin descripción en USO)_'} |")
    out.append("")
    return out, len(filas)


# ---------------------------------------------------------------- MCP

def cierra(texto, i, ab, ce):
    """Índice del cierre que corresponde al `ab` de la posición i (ignora cadenas)."""
    d, en_cad = 0, False
    k = i
    while k < len(texto):
        ch = texto[k]
        if en_cad:
            if ch == "\\":
                k += 1
            elif ch == '"':
                en_cad = False
        elif ch == '"':
            en_cad = True
        elif ch == ab:
            d += 1
        elif ch == ce:
            d -= 1
            if d == 0:
                return k
        k += 1
    return -1


def claves_nivel1(cuerpo):
    claves, d, en_cad, ini = [], 0, False, 0
    k = 0
    trozos = []
    while k < len(cuerpo):
        ch = cuerpo[k]
        if en_cad:
            if ch == "\\":
                k += 1
            elif ch == '"':
                en_cad = False
        elif ch == '"':
            en_cad = True
        elif ch in "{([":
            d += 1
        elif ch in "})]":
            d -= 1
        elif ch == "," and d == 0:
            trozos.append(cuerpo[ini:k])
            ini = k + 1
        k += 1
    trozos.append(cuerpo[ini:])
    for tr in trozos:
        mm = re.match(r'\s*"(\w+)"\s*:', tr)
        if mm:
            claves.append(mm.group(1))
    return claves


def seccion_mcp():
    t = leer(RUTAS["mcp"])
    if t is None:
        return ["_No se encontró `core/src/bin/mesh-mcp.rs`._", ""], 0
    ini = t.find("fn tools()")
    if ini < 0:
        print("AVISO: no encuentro `fn tools()` en mesh-mcp.rs.")
        return ["_No se encontró la lista de herramientas._", ""], 0
    cuerpo = t[ini:]
    out = ["| Herramienta | Parámetros | Qué hace |", "|---|---|---|"]
    n = 0
    for m in re.finditer(r'\btool\(\s*"(\w+)",\s*"((?:[^"\\]|\\.)*)",\s*', cuerpo):
        resto = cuerpo[m.end():]
        params, oblig = [], []
        j = resto.find("json!(")
        if j == 0:
            a = resto.find("{", j)
            b = cierra(resto, a, "{", "}")
            params = claves_nivel1(resto[a + 1 : b])
            tras = resto[b:]
        else:
            tras = resto
        mm = re.match(r"[^&]*&\[([^\]]*)\]", tras)
        if mm:
            oblig = re.findall(r'"(\w+)"', mm.group(1))
        ps = ", ".join(f"`{p}`{'' if p in oblig else ' (opcional)'}" for p in params) or "—"
        desc = m.group(2).replace('\\"', '"')
        out.append(f"| `{m.group(1)}` | {ps} | {esc(desc)} |")
        n += 1
    out.append("")
    return out, n


# ---------------------------------------------------------------- Principal

def main():
    resumen = {}
    cuerpo = []
    a, resumen["Android (funciones y propiedades)"] = seccion_android()
    w, resumen["Web (métodos)"] = seccion_js_web()
    r, resumen["React Native (métodos)"] = seccion_js_rn()
    c, resumen["CLI mesh (órdenes)"] = seccion_cli()
    p, resumen["MCP (herramientas)"] = seccion_mcp()
    cuerpo += ["## SDK de Android (Kotlin)", "", *a]
    cuerpo += ["## SDK web (JavaScript)", "", "Archivo `web/mobilemesh.js`; se importa como módulo (`import { MobileMesh } from \"./mobilemesh.js\"`).", "", *w]
    cuerpo += ["## React Native", "", *r]
    cuerpo += ["## Línea de órdenes `mesh`", "", *c]
    cuerpo += ["## Servidor MCP `mesh-mcp`", "", "Herramientas que expone a una IA (`tools/list`). El nodo de prueba vive dentro de `~/.mobilemesh-mcp`.", "", *p]
    cab = [
        "# Referencia de la API de MobileMesh",
        "",
        "> **Archivo GENERADO. No lo edites a mano.** Se crea leyendo el código fuente.",
        "> Para regenerarlo: `python3 tools/generar-referencia.py`",
        "> Si cambias una función, vuelve a ejecutar el script; los cambios manuales se pierden.",
        "",
        "Resumen: " + " · ".join(f"{k}: {v}" for k, v in resumen.items()) + ".",
        "",
        "Los nombres de Android van en inglés (es la API). En web, React Native, CLI y MCP los nombres son en español, tal como están en el código.",
        "",
    ]
    SALIDA.parent.mkdir(parents=True, exist_ok=True)
    SALIDA.write_text("\n".join(cab + cuerpo).rstrip("\n") + "\n", encoding="utf-8")
    print(f"Escrito {SALIDA.relative_to(RAIZ)}")
    for k, v in resumen.items():
        print(f"  {k}: {v}")


if __name__ == "__main__":
    sys.exit(main())
