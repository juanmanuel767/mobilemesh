// Red de MobileMesh en el navegador (lado de la página). Abre los WebSocket que pide el núcleo y
// deja lo que llega en la memoria compartida de cada conexión (ver core/js/conexion.js).
const TAM = 256 * 1024;

export function iniciarRed(puerto) {
  const conexiones = new Map();
  puerto.onmessage = ({ data: m }) => {
    if (m.t === "abrir") {
      const c = { cab: new Int32Array(m.sab, 0, 4), datos: new Uint8Array(m.sab, 16), pendientes: [], abierta: false, cerrada: false };
      conexiones.set(m.id, c);
      const protocolo = location.protocol === "https:" ? "wss" : "ws";
      try {
        c.ws = new WebSocket(`${protocolo}://${m.direccion}`);
      } catch {
        return marcar(c, 3);
      }
      c.ws.binaryType = "arraybuffer";
      c.ws.onopen = () => {
        c.abierta = true;
        marcar(c, 1);
      };
      c.ws.onmessage = (e) => {
        c.pendientes.push(new Uint8Array(e.data));
        volcar(c);
      };
      c.ws.onclose = c.ws.onerror = () => {
        c.cerrada = true;
        volcar(c);
      };
    } else if (m.t === "enviar") {
      const c = conexiones.get(m.id);
      if (c?.abierta && c.ws.readyState === WebSocket.OPEN) c.ws.send(m.datos);
    } else if (m.t === "cerrar") {
      conexiones.get(m.id)?.ws?.close();
      conexiones.delete(m.id);
    }
  };
}

function marcar(c, estado) {
  Atomics.store(c.cab, 0, estado);
  Atomics.notify(c.cab, 0);
  Atomics.notify(c.cab, 1);
}

// Copia lo recibido al anillo compartido; si el núcleo aún no leyó lo anterior, reintenta enseguida.
function volcar(c) {
  while (c.pendientes.length) {
    const w = Atomics.load(c.cab, 1);
    const libre = TAM - (w - Atomics.load(c.cab, 2));
    if (libre <= 0) return void setTimeout(() => volcar(c), 5);
    const p = c.pendientes[0];
    const n = Math.min(libre, p.length);
    for (let i = 0; i < n; i++) c.datos[(w + i) % TAM] = p[i];
    Atomics.store(c.cab, 1, w + n);
    Atomics.notify(c.cab, 1);
    if (n < p.length) c.pendientes[0] = p.subarray(n);
    else c.pendientes.shift();
  }
  if (c.cerrada && Atomics.load(c.cab, 0) < 2) marcar(c, c.abierta ? 2 : 3);
}
