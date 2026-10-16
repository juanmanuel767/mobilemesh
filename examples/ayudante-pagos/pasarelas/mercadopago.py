"""Mercado Pago (Checkout Pro: Latinoamérica).

claves.env:  MERCADOPAGO_TOKEN=TEST-…   (Access Token; el de pruebas empieza por TEST-)
"""
import json, urllib.parse, urllib.request

API = "https://api.mercadopago.com"
ESTADOS = {"approved": "APROBADO", "rejected": "RECHAZADO", "cancelled": "ANULADO", "refunded": "ANULADO", "charged_back": "ANULADO"}


def _pedir(c, ruta, datos=None):
    req = urllib.request.Request(f"{API}{ruta}", data=json.dumps(datos).encode() if datos else None,
                                 headers={"Authorization": f"Bearer {c['MERCADOPAGO_TOKEN']}", "Content-Type": "application/json"})
    return json.load(urllib.request.urlopen(req, timeout=20))


def crear(pedido, volver, c):
    preferencia = {
        "items": [{"title": pedido["nombre"], "quantity": 1, "unit_price": pedido["precio"], "currency_id": pedido["moneda"]}],
        "external_reference": pedido["referencia"],
    }
    if volver:
        preferencia["back_urls"] = {"success": volver, "failure": volver, "pending": volver}
        preferencia["auto_return"] = "approved"
    r = _pedir(c, "/checkout/preferences", preferencia)
    # Con un token de pruebas, la página de pruebas.
    url = r["sandbox_init_point"] if c["MERCADOPAGO_TOKEN"].startswith("TEST-") else r["init_point"]
    return url, r["id"]


def estado(pedido, c):
    r = _pedir(c, "/v1/payments/search?" + urllib.parse.urlencode({"external_reference": pedido["referencia"]}))
    estados = [ESTADOS.get(p["status"], "PENDIENTE") for p in r.get("results", [])]
    return "APROBADO" if "APROBADO" in estados else (estados[-1] if estados else "PENDIENTE")
