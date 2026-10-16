// SDK web de MobileMesh. Necesita que la página se sirva con las cabeceras
//   Cross-Origin-Opener-Policy: same-origin
//   Cross-Origin-Embedder-Policy: require-corp
// (memoria compartida entre la página y el núcleo).
import { iniciarRed } from "./red.js";

export class MobileMesh {
  static async iniciar() {
    if (!self.crossOriginIsolated) throw new Error("La página necesita las cabeceras COOP y COEP (ver mobilemesh.js)");
    const canal = new MessageChannel();
    iniciarRed(canal.port1);
    const worker = new Worker(new URL("./nucleo-worker.js", import.meta.url), { type: "module" });
    const m = new MobileMesh(worker);
    await new Promise((listo, falla) => {
      worker.onmessage = ({ data }) => (data.t !== "listo" ? m.#respuesta(data) : data.error ? falla(new Error(data.error)) : listo());
      worker.postMessage({ t: "iniciar", red: canal.port2 }, [canal.port2]);
    });
    worker.onmessage = ({ data }) => m.#respuesta(data);
    return m;
  }

  #worker;
  #pendientes = new Map();
  #siguiente = 1;

  constructor(worker) {
    this.#worker = worker;
  }

  #alNuevo = null;

  #respuesta({ t, n, id, ok, error }) {
    if (t === "nuevos") return this.#alNuevo?.(n);
    const p = this.#pendientes.get(id);
    this.#pendientes.delete(id);
    error === undefined ? p?.resolve(ok) : p?.reject(new Error(error));
  }

  #llamar(fn, ...args) {
    const id = this.#siguiente++;
    return new Promise((resolve, reject) => {
      this.#pendientes.set(id, { resolve, reject });
      this.#worker.postMessage({ t: "llamar", id, fn, args });
    });
  }

  /** Identificador de la cuenta de este navegador (se crea la primera vez y se guarda). */
  cuenta() {
    return this.#llamar("cuenta");
  }

  /** Guarda un campo de un registro de una colección (cifrado y permanente). */
  guardar(coleccion, registro, campo, valor) {
    return this.#llamar("guardar_dato", coleccion, registro, campo, String(valor));
  }

  /** Registros de una colección: [{id, campos: {…}}]. */
  async datos(coleccion) {
    return JSON.parse(await this.#llamar("ver_datos", coleccion));
  }

  /** Mi invitación (para que otro me añada como contacto). */
  invitacion() {
    return this.#llamar("invitacion");
  }

  anadirContacto(nombre, invitacion) {
    return this.#llamar("anadir_contacto", nombre, invitacion);
  }

  /** Envía un texto. Responde «entregado», «en buzón» o «pendiente: …». */
  enviar(nombre, texto) {
    return this.#llamar("enviar", nombre, texto);
  }

  /** Todos los mensajes: [{contacto, saliente, texto, estado}]. */
  async mensajes() {
    return JSON.parse(await this.#llamar("mensajes"));
  }

  /** Empieza a recibir (por el relé) mientras la página esté abierta. `alNuevo(n)` avisa. */
  recibir(alNuevo) {
    this.#alNuevo = alNuevo;
    this.#worker.postMessage({ t: "recibir" });
  }

  /** Comparte una colección con un contacto: "leer", "escribir" o "nada" (quitar). */
  darPermiso(coleccion, contacto, permiso) {
    return this.#llamar("dar_permiso", coleccion, contacto, permiso);
  }

  /** Sincroniza con un contacto las colecciones compartidas. */
  sincronizar(contacto) {
    return this.#llamar("sincronizar", contacto);
  }

  /** Mi IP pública según un relé («host:puerto»). */
  quien(rele) {
    return this.#llamar("quien", rele);
  }

  /** Directorio firmado de relés y guardianes que sirve un relé (comprobada la firma). */
  async directorio(rele) {
    return JSON.parse(await this.#llamar("directorio", rele));
  }
}
