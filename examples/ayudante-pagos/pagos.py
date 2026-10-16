#!/usr/bin/env python3
"""Ayudante de pagos para MobileMesh (funciones de servidor), con cualquier pasarela.

Lo común vive aquí: catálogo (el precio lo pone el ayudante, no el móvil), pedidos, quién puede ver
cada pago y la dirección de regreso a la app. Lo propio de cada pasarela vive en `pasarelas/<nombre>.py`
con dos funciones (ver pasarelas/README.md):

    crear(pedido, volver, claves) -> (url_de_pago, id_externo)
    estado(pedido, claves)        -> "APROBADO" | "RECHAZADO" | "ANULADO" | "ERROR" | "PENDIENTE"

Las claves viven SOLO aquí (claves.env, fuera de git), nunca en la app.

Uso:  MM_CLAVES=/ruta/claves.env  mesh <carpeta> ayudante <ip:puerto> /ruta/pagos.py
claves.env:  PASARELA=wompi | mercadopago | stripe   (+ las claves de esa pasarela)

Funciones (desde la app: MobileMesh.pagar("producto"), o llamar("pagos", <función>, datos)):
  productos                       → [{id, nombre, precio}]
  crear_pago {producto, volver?}  → {referencia, url}
  estado_pago {referencia}        → {estado}
"""
import importlib, json, os, re, secrets, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

# ponytail: catálogo fijo de prueba; leerlo de una colección de MobileMesh cuando haya tienda de verdad.
MONEDA = "COP"
CATALOGO = {
    "cafe": ("Café", 5_000),
    "camiseta": ("Camiseta ChatMesh", 45_000),
}


def claves():
    c = {}
    for linea in Path(os.environ["MM_CLAVES"]).read_text().splitlines():
        if "=" in linea and not linea.lstrip().startswith("#"):
            k, v = linea.split("=", 1)
            c[k.strip()] = v.strip()
    return c


# Pedidos: referencia → {cuenta, producto, …}. Solo la cuenta que lo creó puede consultarlo.
PEDIDOS = Path(os.environ["MM_CLAVES"]).with_name("pedidos.json")


def pedidos():
    return json.loads(PEDIDOS.read_text()) if PEDIDOS.exists() else {}


def guardar(p):
    PEDIDOS.write_text(json.dumps(p, indent=1, ensure_ascii=False))


def main():
    funcion = sys.argv[1]
    entrada = json.load(sys.stdin)
    cuenta, datos = entrada["de"], entrada.get("datos") or {}
    c = claves()
    pasarela = importlib.import_module(f"pasarelas.{c.get('PASARELA', 'wompi')}")

    if funcion == "productos":
        return [{"id": k, "nombre": n, "precio": p, "moneda": MONEDA, "precio_cop": p} for k, (n, p) in CATALOGO.items()]

    if funcion == "crear_pago":
        if datos.get("producto") not in CATALOGO:
            raise ValueError("ese producto no existe")
        nombre, precio = CATALOGO[datos["producto"]]
        # Dirección de regreso: solo «esquema://…» sencillo, nada de javascript:, data:…
        volver = datos.get("volver")
        if volver and (not re.fullmatch(r"[a-z][a-z0-9+.-]*://[\w./?=&%-]*", volver) or volver.startswith(("javascript", "data", "file"))):
            raise ValueError("dirección de regreso no válida")
        referencia = f"cm-{cuenta[:8]}-{secrets.token_hex(6)}"
        pedido = {"referencia": referencia, "cuenta": cuenta, "producto": datos["producto"], "nombre": nombre,
                  "precio": precio, "centavos": precio * 100, "moneda": MONEDA, "estado": "PENDIENTE"}
        url, pedido["id_externo"] = pasarela.crear(pedido, volver, c)
        p = pedidos()
        p[referencia] = pedido
        guardar(p)
        return {"referencia": referencia, "url": url, "producto": nombre, "precio": precio, "precio_cop": precio}

    if funcion == "estado_pago":
        p = pedidos()
        pedido = p.get(datos.get("referencia", ""))
        if not pedido or pedido["cuenta"] != cuenta:
            raise ValueError("no tienes un pago con esa referencia")
        if pedido["estado"] == "PENDIENTE":
            pedido["estado"] = pasarela.estado(pedido, c)
            guardar(p)
        return {"estado": pedido["estado"]}

    raise ValueError(f"no existe la función {funcion}")


if __name__ == "__main__":
    try:
        print(json.dumps(main(), ensure_ascii=False))
    except Exception as e:  # el mensaje vuelve a la app como error de `llamar`
        print(str(e), file=sys.stderr)
        sys.exit(1)
