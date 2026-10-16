#!/usr/bin/env python3
"""Ayudante de pagos con Wompi para MobileMesh (funciones de servidor).

La clave privada y el secreto de integridad viven SOLO aquí (nunca en la app). El precio lo pone
este ayudante (catálogo), no el móvil: así nadie paga 1 peso por un producto de 50 000.

Uso:  MM_CLAVES=/ruta/claves.env  mesh <carpeta> ayudante <ip:puerto> /ruta/pagos.py
claves.env (fuera de git):  WOMPI_PUBLICA, WOMPI_PRIVADA, WOMPI_INTEGRIDAD, WOMPI_API
  (sandbox: https://sandbox.wompi.co/v1 · producción: https://production.wompi.co/v1)

Funciones (desde la app: mesh.llamar("pagos", <función>, datos)):
  productos                 → [{id, nombre, precio_cop}]
  crear_pago {producto, volver?} → {referencia, url}   (abrir la url: pago de Wompi; al terminar,
                            Wompi abre «volver», p. ej. «miapp://pago», para regresar a la app)
  estado_pago {referencia}  → {estado}            PENDIENTE | APROBADO | RECHAZADO | ANULADO | ERROR
"""
import datetime, hashlib, json, os, re, secrets, sys, urllib.parse, urllib.request
from pathlib import Path

# ponytail: catálogo fijo de prueba; leerlo de una colección de MobileMesh cuando haya tienda de verdad.
CATALOGO = {
    "cafe": ("Café", 5_000),
    "camiseta": ("Camiseta ChatMesh", 45_000),
}
ESTADOS = {"PENDING": "PENDIENTE", "APPROVED": "APROBADO", "DECLINED": "RECHAZADO", "VOIDED": "ANULADO", "ERROR": "ERROR"}


def claves():
    c = {}
    for linea in Path(os.environ["MM_CLAVES"]).read_text().splitlines():
        if "=" in linea:
            k, v = linea.split("=", 1)
            c[k.strip()] = v.strip()
    return c


# Pedidos: referencia → cuenta que lo pidió (solo su dueño puede consultar su estado).
PEDIDOS = Path(os.environ["MM_CLAVES"]).with_name("pedidos.json")


def pedidos():
    return json.loads(PEDIDOS.read_text()) if PEDIDOS.exists() else {}


def main():
    funcion = sys.argv[1]
    entrada = json.load(sys.stdin)
    cuenta, datos = entrada["de"], entrada.get("datos") or {}
    c = claves()

    if funcion == "productos":
        return [{"id": k, "nombre": n, "precio_cop": p} for k, (n, p) in CATALOGO.items()]

    if funcion == "crear_pago":
        if datos.get("producto") not in CATALOGO:
            raise ValueError("ese producto no existe")
        nombre, precio = CATALOGO[datos["producto"]]
        centavos = precio * 100
        referencia = f"cm-{cuenta[:8]}-{secrets.token_hex(6)}"
        # Firma de integridad de Wompi: SHA-256 de referencia + monto en centavos + moneda + secreto.
        firma = hashlib.sha256(f"{referencia}{centavos}COP{c['WOMPI_INTEGRIDAD']}".encode()).hexdigest()
        parametros = {
            "public-key": c["WOMPI_PUBLICA"],
            "currency": "COP",
            "amount-in-cents": centavos,
            "reference": referencia,
            "signature:integrity": firma,
        }
        # Dirección de regreso (no entra en la firma): solo «esquema://…» sencillo, nada de javascript:.
        volver = datos.get("volver")
        if volver:
            if not re.fullmatch(r"[a-z][a-z0-9+.-]*://[\w./?=&%-]*", volver) or volver.startswith(("javascript", "data", "file")):
                raise ValueError("dirección de regreso no válida")
            parametros["redirect-url"] = volver
        url = "https://checkout.wompi.co/p/?" + urllib.parse.urlencode(parametros)
        p = pedidos()
        p[referencia] = {"cuenta": cuenta, "producto": datos["producto"], "precio_cop": precio, "estado": "PENDIENTE"}
        PEDIDOS.write_text(json.dumps(p, indent=1))
        return {"referencia": referencia, "url": url, "producto": nombre, "precio_cop": precio}

    if funcion == "estado_pago":
        p = pedidos()
        pedido = p.get(datos.get("referencia", ""))
        if not pedido or pedido["cuenta"] != cuenta:
            raise ValueError("no tienes un pago con esa referencia")
        hoy = datetime.date.today()
        q = urllib.parse.urlencode({
            "reference": datos["referencia"], "page": 1, "page_size": 10,
            "from_date": (hoy - datetime.timedelta(days=2)).isoformat(),
            "until_date": (hoy + datetime.timedelta(days=1)).isoformat(),
        })
        req = urllib.request.Request(f"{c['WOMPI_API']}/transactions?{q}", headers={"Authorization": f"Bearer {c['WOMPI_PRIVADA']}"})
        transacciones = json.load(urllib.request.urlopen(req, timeout=20)).get("data", [])
        # Aprobado si alguna transacción de esa referencia se aprobó; si no, la más reciente.
        estados = [ESTADOS.get(t["status"], t["status"]) for t in transacciones]
        estado = "APROBADO" if "APROBADO" in estados else (estados[0] if estados else "PENDIENTE")
        pedido["estado"] = estado
        PEDIDOS.write_text(json.dumps(p, indent=1))
        return {"estado": estado}

    raise ValueError(f"no existe la función {funcion}")


if __name__ == "__main__":
    try:
        print(json.dumps(main(), ensure_ascii=False))
    except Exception as e:  # el mensaje vuelve a la app como error de `llamar`
        print(str(e), file=sys.stderr)
        sys.exit(1)
