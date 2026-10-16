"""Stripe (Checkout: tarjetas, Apple Pay y más, en muchos países).

claves.env:  STRIPE_SECRETA=sk_test_…
"""
import base64, json, urllib.parse, urllib.request

API = "https://api.stripe.com/v1"


def _pedir(c, ruta, datos=None):
    req = urllib.request.Request(f"{API}{ruta}", data=urllib.parse.urlencode(datos).encode() if datos else None,
                                 headers={"Authorization": "Basic " + base64.b64encode(f"{c['STRIPE_SECRETA']}:".encode()).decode()})
    return json.load(urllib.request.urlopen(req, timeout=20))


def crear(pedido, volver, c):
    # Sin «volver», Stripe exige igualmente una dirección: la página puente sin app solo muestra un mensaje.
    destino = volver or "https://juanmanuel767.github.io/mobilemesh/volver/"
    r = _pedir(c, "/checkout/sessions", {
        "mode": "payment",
        "client_reference_id": pedido["referencia"],
        "line_items[0][quantity]": 1,
        "line_items[0][price_data][currency]": pedido["moneda"].lower(),
        "line_items[0][price_data][unit_amount]": pedido["centavos"],
        "line_items[0][price_data][product_data][name]": pedido["nombre"],
        "success_url": destino,
        "cancel_url": destino,
    })
    return r["url"], r["id"]


def estado(pedido, c):
    r = _pedir(c, f"/checkout/sessions/{pedido['id_externo']}")
    if r.get("payment_status") == "paid":
        return "APROBADO"
    return "ANULADO" if r.get("status") == "expired" else "PENDIENTE"
