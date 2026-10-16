"""Wompi (Colombia: tarjeta, Nequi, PSE, Bancolombia, Daviplata…).

claves.env:  WOMPI_PUBLICA, WOMPI_PRIVADA, WOMPI_INTEGRIDAD,
             WOMPI_API=https://sandbox.wompi.co/v1   (producción: https://production.wompi.co/v1)
"""
import datetime, hashlib, json, urllib.parse, urllib.request

ESTADOS = {"APPROVED": "APROBADO", "DECLINED": "RECHAZADO", "VOIDED": "ANULADO", "ERROR": "ERROR", "PENDING": "PENDIENTE"}


def crear(pedido, volver, c):
    # Firma de integridad: SHA-256 de referencia + monto en centavos + moneda + secreto.
    firma = hashlib.sha256(f"{pedido['referencia']}{pedido['centavos']}{pedido['moneda']}{c['WOMPI_INTEGRIDAD']}".encode()).hexdigest()
    parametros = {
        "public-key": c["WOMPI_PUBLICA"],
        "currency": pedido["moneda"],
        "amount-in-cents": pedido["centavos"],
        "reference": pedido["referencia"],
        "signature:integrity": firma,
    }
    if volver:
        parametros["redirect-url"] = volver  # Wompi solo acepta https://
    return "https://checkout.wompi.co/p/?" + urllib.parse.urlencode(parametros), pedido["referencia"]


def estado(pedido, c):
    hoy = datetime.date.today()
    q = urllib.parse.urlencode({
        "reference": pedido["referencia"], "page": 1, "page_size": 10,
        "from_date": (hoy - datetime.timedelta(days=2)).isoformat(),
        "until_date": (hoy + datetime.timedelta(days=1)).isoformat(),
    })
    req = urllib.request.Request(f"{c['WOMPI_API']}/transactions?{q}", headers={"Authorization": f"Bearer {c['WOMPI_PRIVADA']}"})
    estados = [ESTADOS.get(t["status"], "PENDIENTE") for t in json.load(urllib.request.urlopen(req, timeout=20)).get("data", [])]
    # Aprobado si alguna transacción de esa referencia se aprobó; si no, la más reciente.
    return "APROBADO" if "APROBADO" in estados else (estados[0] if estados else "PENDIENTE")
