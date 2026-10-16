# Ayudante de pagos con Wompi

Cobrar desde una app MobileMesh con [Wompi](https://wompi.com) (Colombia: tarjeta, Nequi, PSE,
Bancolombia…). Las claves de Wompi viven **solo** en el ayudante, nunca en la app, y el precio lo
pone el ayudante, no el móvil.

```
App ── llamar("pagos","crear_pago",{producto}) ──▶ ayudante: firma el pago con el secreto de integridad
App ◀──────────── {url, referencia} ─────────────
App abre la url (pago de Wompi) ──────────────────────────────────────────────▶ Wompi
App ── llamar("pagos","estado_pago",{referencia}) ─▶ ayudante: pregunta a Wompi con la clave privada
App ◀──────────── {estado: "APROBADO"} ──────────
```

## Poner en marcha

1. `claves.env` **fuera del repositorio** (permisos 600):
   ```
   WOMPI_PUBLICA=pub_test_…
   WOMPI_PRIVADA=prv_test_…
   WOMPI_INTEGRIDAD=test_integrity_…
   WOMPI_API=https://sandbox.wompi.co/v1      # producción: https://production.wompi.co/v1
   ```
2. Cuenta del ayudante y su nombre en la guía:
   `mesh pagos-mesh init` y `mesh pagos-mesh registrar-nombre <guía> pagos <ip:puerto>`
3. Arrancar: `MM_CLAVES=/ruta/claves.env mesh pagos-mesh ayudante <ip:puerto> examples/ayudante-wompi/pagos.py`
4. En la app (React Native), una línea:
   ```js
   const { estado } = await MobileMesh.pagar("cafe");   // "APROBADO" | "RECHAZADO" | …
   ```
   `pagar()` añade el ayudante, crea el pago, abre Wompi y espera el resultado. Para volver a la app,
   Wompi (que solo acepta https) va a la página puente `docs/volver` (GitHub Pages), que abre
   `<id de la app>://pago`; el kit trae la pantalla que lo recibe, así que la app no declara nada.

El catálogo está en `CATALOGO` dentro de `pagos.py`. Los pedidos quedan en `pedidos.json`, junto a
`claves.env`. Solo la cuenta que creó un pago puede consultar su estado.

## Probado (modo de pruebas)

- Productos, crear pago, firma aceptada por Wompi (página de pago con el importe correcto), estado.
- De principio a fin en un móvil real (Redmi, app ChatMesh): pago con tarjeta de prueba 4242…,
  Wompi lo aprueba y la app muestra «Pagado» al volver.
- Rechazado: producto inexistente y consultar el pago de otra cuenta.

## Pendiente

- Avisos automáticos de Wompi (eventos con firma) en lugar de preguntar cada 5 s: necesita una
  dirección pública https para el ayudante.
