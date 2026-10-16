// Conexiones del núcleo en el navegador (lado del núcleo, dentro de su trabajador).
// El núcleo espera bloqueado (Atomics.wait) como en el móvil; la red de verdad la hace red.js en la
// página, con WebSocket, y deja los bytes en memoria compartida. `self.mmRed` es el canal hacia ella.
//
// Cabecera (Int32): [0] estado: 0 abriendo, 1 abierta, 2 cerrada, 3 error
//                   [1] bytes escritos en total · [2] bytes leídos en total
const TAM = 256 * 1024;
let siguiente = 1;
const conexiones = new Map();

export function mm_abrir(direccion, ms) {
  const sab = new SharedArrayBuffer(16 + TAM);
  const cab = new Int32Array(sab, 0, 4);
  const id = siguiente++;
  conexiones.set(id, { cab, datos: new Uint8Array(sab, 16) });
  self.mmRed.postMessage({ t: "abrir", id, direccion, sab });
  Atomics.wait(cab, 0, 0, ms);
  if (Atomics.load(cab, 0) !== 1) {
    mm_cerrar(id);
    return -1;
  }
  return id;
}

// >0: bytes leídos · 0: cerrada · -1: tiempo agotado · -2: error
export function mm_leer(id, buf, ms) {
  const c = conexiones.get(id);
  if (!c) return -2;
  const limite = ms ? Date.now() + ms : Infinity;
  for (;;) {
    const w = Atomics.load(c.cab, 1);
    const r = Atomics.load(c.cab, 2);
    if (w > r) {
      const n = Math.min(w - r, buf.length);
      for (let i = 0; i < n; i++) buf[i] = c.datos[(r + i) % TAM];
      Atomics.store(c.cab, 2, r + n);
      return n;
    }
    const estado = Atomics.load(c.cab, 0);
    if (estado >= 2) return estado === 2 ? 0 : -2;
    const resto = limite - Date.now();
    if (resto <= 0) return -1;
    Atomics.wait(c.cab, 1, w, resto === Infinity ? undefined : resto);
  }
}

export function mm_escribir(id, datos) {
  const c = conexiones.get(id);
  if (!c || Atomics.load(c.cab, 0) !== 1) return -2;
  self.mmRed.postMessage({ t: "enviar", id, datos: datos.slice() });
  return datos.length;
}

export function mm_cerrar(id) {
  if (conexiones.delete(id)) self.mmRed.postMessage({ t: "cerrar", id });
}
