# Ayudante de pagos (cualquier pasarela)

Cobrar desde una app MobileMesh con **Wompi, Mercado Pago, Stripe** o la pasarela que quieras. Las
claves viven **solo** en el ayudante, nunca en la app, y el precio lo pone el ayudante, no el móvil.

```
App: MobileMesh.pagar("cafe")
  └─► ayudante «pagos»  (pagos.py: catálogo, pedidos, quién ve cada pago)
        └─► pasarelas/<PASARELA>.py ─► Wompi · Mercado Pago · Stripe · …
  ◀── página de pago ── la pasarela regresa a la app (página puente docs/volver) ── {estado: "APROBADO"}
```

En la app, una línea:

```js
const { estado } = await MobileMesh.pagar("cafe");   // "APROBADO" | "RECHAZADO" | "ANULADO" | "ERROR" | "PENDIENTE"
```

`pagar()` añade el ayudante, crea el pago, abre la página de la pasarela, vuelve sola a la app y
espera el resultado. La app no declara nada en Android: el regreso lo trae el kit.

## Poner en marcha

1. `claves.env` **fuera del repositorio** (permisos 600). Elige la pasarela y pon sus claves:
   ```
   PASARELA=wompi                 # o mercadopago, stripe…
   WOMPI_PUBLICA=pub_test_…
   WOMPI_PRIVADA=prv_test_…
   WOMPI_INTEGRIDAD=test_integrity_…
   WOMPI_API=https://sandbox.wompi.co/v1
   ```
   Las claves de cada pasarela: [pasarelas/README.md](pasarelas/README.md).
2. Cuenta del ayudante y su nombre en la guía de nombres:
   `mesh pagos-mesh init` y `mesh pagos-mesh registrar-nombre <guía> pagos <ip:puerto>`
3. Arrancar: `MM_CLAVES=/ruta/claves.env mesh pagos-mesh ayudante <ip:puerto> examples/ayudante-pagos/pagos.py`

El catálogo está en `CATALOGO` dentro de `pagos.py`. Los pedidos quedan en `pedidos.json`, junto a
`claves.env`. Solo la cuenta que creó un pago puede consultar su estado.

## Otra pasarela

Un archivo en `pasarelas/` con dos funciones, `crear()` y `estado()`: [pasarelas/README.md](pasarelas/README.md).

## Probado

- Wompi (modo de pruebas), de principio a fin en un móvil real (Redmi, app ChatMesh): pago con
  tarjeta de prueba, Wompi lo aprueba y la app vuelve sola y muestra «Pagado».
- Rechazado: producto inexistente, dirección de regreso no válida y consultar el pago de otra cuenta.
- Mercado Pago y Stripe: conectadas (cada una rechaza claves falsas); falta probarlas con claves de prueba.

## Pendiente

- Avisos automáticos de la pasarela (webhooks) en lugar de preguntar cada 4 s: necesita una
  dirección pública https para el ayudante.
