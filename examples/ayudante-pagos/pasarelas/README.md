# Añadir una pasarela de pago

Cada pasarela es **un archivo** en esta carpeta con dos funciones. La app y el kit no cambian:
siguen llamando a `MobileMesh.pagar("producto")`.

```python
def crear(pedido, volver, claves) -> tuple[str, str]:
    """Crea el pago en la pasarela. Devuelve (url_de_la_página_de_pago, id_externo)."""

def estado(pedido, claves) -> str:
    """Pregunta a la pasarela. Devuelve "APROBADO", "RECHAZADO", "ANULADO", "ERROR" o "PENDIENTE"."""
```

`pedido` trae: `referencia` (única, úsala como referencia en la pasarela), `nombre`, `precio`,
`centavos`, `moneda`, `id_externo` (lo que devolvió `crear`) y `cuenta`.
`volver` es la dirección https a la que la pasarela debe regresar al terminar (puede ser `None`).
`claves` son las líneas de `claves.env`.

Para usarla: `PASARELA=<nombre del archivo>` en `claves.env`. Sirve cualquier pasarela con página
de pago alojada y una forma de consultar el estado (casi todas: PayU, ePayco, Bold, PayPal…).

| Pasarela | Archivo | Claves | Estado |
|---|---|---|---|
| Wompi | `wompi.py` | `WOMPI_PUBLICA`, `WOMPI_PRIVADA`, `WOMPI_INTEGRIDAD`, `WOMPI_API` | ✔ probada en móvil real (modo de pruebas) |
| Mercado Pago | `mercadopago.py` | `MERCADOPAGO_TOKEN` | conectada (rechaza claves falsas); falta probar con claves de prueba |
| Stripe | `stripe.py` | `STRIPE_SECRETA` | conectada (rechaza claves falsas); falta probar con claves de prueba |
