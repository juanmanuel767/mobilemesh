// SDK web de MobileMesh. Necesita que la página se sirva con las cabeceras
//   Cross-Origin-Opener-Policy: same-origin
//   Cross-Origin-Embedder-Policy: require-corp
// (memoria compartida entre la página y el núcleo).
import { iniciarRed } from "./red.js";

// Alias en español obsoletos: cada uno avisa una sola vez por nombre.
const avisados = new Set();
function obsoleto(viejo, nuevo) {
  if (avisados.has(viejo)) return;
  avisados.add(viejo);
  console.warn(`MobileMesh: ${viejo}() está obsoleto, usa ${nuevo}()`);
}

// Permisos aceptados -> lo que espera el núcleo ("leer" | "escribir" | "nada").
const PERMISOS = { read: "leer", write: "escribir", none: "nada", leer: "leer", escribir: "escribir", nada: "nada" };

export class MobileMesh {
  /**
   * Starts MobileMesh in this page: loads the core in a worker and connects the network.
   * Arranca MobileMesh en la página: carga el núcleo en un trabajador y conecta la red.
   */
  static async start() {
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
    if (t === "aviso") return console.warn("MobileMesh (recibir):", arguments[0].error);
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

  /**
   * Identifier of this browser's account (created the first time and kept).
   * Identificador de la cuenta de este navegador (se crea la primera vez y se guarda).
   */
  account() {
    return this.#llamar("cuenta");
  }

  /**
   * Saves one field of a record in a collection (encrypted and permanent).
   * Guarda un campo de un registro de una colección (cifrado y permanente).
   */
  save(collection, record, field, value) {
    return this.#llamar("guardar_dato", collection, record, field, String(value));
  }

  /**
   * Records of a collection: [{id, campos: {…}}].
   * Registros de una colección: [{id, campos: {…}}].
   */
  async all(collection) {
    return JSON.parse(await this.#llamar("ver_datos", collection));
  }

  /**
   * My invitation (so another person can add me as a contact).
   * Mi invitación (para que otro me añada como contacto).
   */
  invitation() {
    return this.#llamar("invitacion");
  }

  /**
   * Adds a contact under a name, using the invitation they shared.
   * Añade un contacto con un nombre, a partir de la invitación que compartió.
   */
  addContact(name, invitation) {
    return this.#llamar("anadir_contacto", name, invitation);
  }

  /**
   * Sends a text. Resolves "entregado", "en buzón" or "pendiente: …".
   * Envía un texto. Responde «entregado», «en buzón» o «pendiente: …».
   */
  sendMessage(name, text) {
    return this.#llamar("enviar", name, text);
  }

  /**
   * All messages: [{contacto, saliente, texto, estado}].
   * Todos los mensajes: [{contacto, saliente, texto, estado}].
   */
  async messages() {
    return JSON.parse(await this.#llamar("mensajes"));
  }

  /**
   * Starts receiving (through the relay) while the page is open. `callback(n)` is called on new messages.
   * Empieza a recibir (por el relé) mientras la página esté abierta. `callback(n)` avisa de los nuevos.
   */
  onReceive(callback) {
    this.#alNuevo = callback;
    this.#worker.postMessage({ t: "recibir" });
  }

  /**
   * Shares a collection with a contact: "read", "write" or "none" (revoke). Spanish "leer", "escribir", "nada" also work.
   * Comparte una colección con un contacto: "read", "write" o "none" (quitar). También valen "leer", "escribir", "nada".
   */
  async shareWith(collection, contact, permission) {
    const permiso = Object.hasOwn(PERMISOS, permission) ? PERMISOS[permission] : undefined;
    if (permiso === undefined) {
      throw new Error(`MobileMesh: permiso inválido (${String(permission)}); usa "read" | "write" | "none" / invalid permission`);
    }
    return this.#llamar("dar_permiso", collection, contact, permiso);
  }

  /**
   * Syncs the shared collections with a contact.
   * Sincroniza con un contacto las colecciones compartidas.
   */
  syncWith(contact) {
    return this.#llamar("sincronizar", contact);
  }

  /**
   * My public IP as seen by a relay ("host:port").
   * Mi IP pública según un relé («host:puerto»).
   */
  publicAddress(relay) {
    return this.#llamar("quien", relay);
  }

  /**
   * Signed directory of relays and guardians served by a relay (signature checked).
   * Directorio firmado de relés y guardianes que sirve un relé (comprobada la firma).
   */
  async directory(relay) {
    return JSON.parse(await this.#llamar("directorio", relay));
  }
}

// Alias en español (obsoletos). Fuera de la clase a propósito: la API publicada es la inglesa.
// Cada uno es una línea que avisa una vez y llama al nombre inglés, con los mismos argumentos.
MobileMesh.iniciar = function iniciar() { obsoleto("iniciar", "start"); return MobileMesh.start(); };
const alias = {
  cuenta: ["account", function cuenta() { return this.account(); }],
  guardar: ["save", function guardar(coleccion, registro, campo, valor) { return this.save(coleccion, registro, campo, valor); }],
  datos: ["all", function datos(coleccion) { return this.all(coleccion); }],
  invitacion: ["invitation", function invitacion() { return this.invitation(); }],
  anadirContacto: ["addContact", function anadirContacto(nombre, invitacion) { return this.addContact(nombre, invitacion); }],
  enviar: ["sendMessage", function enviar(nombre, texto) { return this.sendMessage(nombre, texto); }],
  mensajes: ["messages", function mensajes() { return this.messages(); }],
  recibir: ["onReceive", function recibir(alNuevo) { return this.onReceive(alNuevo); }],
  darPermiso: ["shareWith", function darPermiso(coleccion, contacto, permiso) { return this.shareWith(coleccion, contacto, permiso); }],
  sincronizar: ["syncWith", function sincronizar(contacto) { return this.syncWith(contacto); }],
  quien: ["publicAddress", function quien(rele) { return this.publicAddress(rele); }],
  directorio: ["directory", function directorio(rele) { return this.directory(rele); }],
};
for (const [viejo, [nuevo, fn]] of Object.entries(alias)) {
  Object.defineProperty(MobileMesh.prototype, viejo, {
    configurable: true,
    writable: true,
    value: function (...args) { obsoleto(viejo, nuevo); return fn.apply(this, args); },
  });
}
