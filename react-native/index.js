// MobileMesh para React Native (Android): base de datos, mensajes, archivos y avisos sin servidores.
import { AppState, Linking, NativeEventEmitter, NativeModules } from "react-native";

const N = NativeModules.MobileMesh;
if (!N) throw new Error("MobileMesh: módulo nativo no encontrado. ¿Recompilaste la app tras instalarlo? (solo Android)");
const eventos = new NativeEventEmitter(N);

// Alias en español obsoletos: cada uno avisa una sola vez por nombre.
const avisados = new Set();
function obsoleto(viejo, nuevo) {
  if (avisados.has(viejo)) return;
  avisados.add(viejo);
  console.warn(`MobileMesh: ${viejo}() está obsoleto, usa ${nuevo}()`);
}

export const MobileMesh = {
  /** "sin cuenta" | "bloqueado" | "lista" */
  state: () => N.estadoCuenta(),
  /** Abre (o crea la primera vez) la cuenta de este móvil y pide los permisos de avisos. Devuelve su id. */
  start: () => N.iniciar(),

  // Usuarios por nombre, como en cualquier base de datos.
  /** Tras `start`: registra el usuario (y guarda la cuenta en el guardián). Responde un aviso o null. */
  register: (username, password) => N.registrar(username, password),
  /** En un móvil nuevo, en lugar de `start`: recupera la cuenta con usuario y contraseña. */
  signIn: (username, password) => N.entrar(username, password),
  myUsername: () => N.miUsuario(),
  /** Código en papel (56 letras): enséñalo para apuntarlo; recupera la cuenta sin contraseña. */
  paperCode: () => N.codigoPapel(),
  /** Contraseña olvidada: entra con el código en papel y elige una contraseña nueva. */
  recoverWithPaper: (username, code, newPassword) => N.entrarConCodigo(username, code, newPassword),
  userExists: (username) => N.existeUsuario(username),
  /** ¿Se puede recuperar la cuenta al formatear? (tiene copia en el guardián, que se actualiza sola) */
  isProtected: () => N.protegida(),
  /** Protege la cuenta si al registrarse no se pudo. */
  protectWithGuardian: (password) => N.proteger(password),
  /** Añade a alguien por su nombre de usuario; su app te añadirá sola. */
  addByName: (username) => N.anadirPorNombre(username),
  /** Acepta a quienes quieren hablar contigo. Devuelve sus nombres. */
  acceptRequests: () => N.aceptarSolicitudes(),
  /** Actualiza tu invitación en la guía y acepta solicitudes. Llámalo al abrir la app y cada poco. */
  refresh: () => N.actualizar(),
  useNameGuide: (invitation) => N.usarGuiaDeNombres(invitation ?? null),

  /** Texto para que otro te añada (compártelo o ponlo en un QR). */
  invitation: () => N.invitacion(),
  addContact: (name, invitation) => N.anadirContacto(name, invitation),
  contacts: () => N.contactos(),
  /** Envía un texto. Responde «entregado» o «en buzón». */
  sendMessage: (contact, text) => N.enviar(contact, text),
  /** [{id, contacto, enviado, texto, estado, progreso, archivo, tipo, tamano, disponible}] */
  messages: async () => JSON.parse(await N.mensajes()),
  /** Avisa con `callback(n)` cada vez que llega algo. Devuelve la función para dejar de escuchar. */
  onReceive: (callback) => {
    const s = eventos.addListener("MobileMeshNuevos", callback);
    return () => s.remove();
  },
  /** Selector de archivos de Android: {uri, nombre} o null. */
  pickFile: () => N.elegirArchivo(),
  sendFile: (contact, uri, name) => N.enviarArchivo(contact, uri, name),
  /** Ruta file:// descifrada en la caché (para <Image>). */
  viewFile: (id) => N.verArchivo(id),
  saveToDownloads: (id) => N.guardarEnDescargas(id),
  /**
   * Llama a una función de servidor de un ayudante (añádelo antes con `addByName`). El ayudante
   * sabe con certeza qué cuenta llama. Devuelve su resultado (objeto si es JSON).
   * Ejemplo de pagos con Wompi: examples/ayudante-pagos.
   */
  call: async (helper, fn, data = {}) => {
    const r = await N.llamar(helper, fn, JSON.stringify(data));
    try {
      return JSON.parse(r);
    } catch {
      return r;
    }
  },
  /**
   * Cobrar con un ayudante de pagos (examples/ayudante-pagos): crea el pago, abre la pasarela, vuelve
   * sola a la app al terminar y espera el resultado. Devuelve {referencia, estado}: estado
   * "APROBADO" | "RECHAZADO" | "ANULADO" | "ERROR", o "PENDIENTE" si en `esperarMin` no se resolvió.
   * La app no tiene que declarar nada: el regreso lo trae el kit.
   */
  pay: async (producto, { ayudante = "pagos", esperarMin = 10 } = {}) => {
    if (!(await N.contactos()).includes(ayudante)) await N.anadirPorNombre(ayudante);
    const volver = `https://juanmanuel767.github.io/mobilemesh/volver/?app=${await N.esquemaDeRegreso()}`;
    const { referencia, url } = await MobileMesh.call(ayudante, "crear_pago", { producto, volver });
    await Linking.openURL(url);
    return new Promise((resolve) => {
      const hasta = Date.now() + esperarMin * 60000;
      let listo = false;
      const mirar = async () => {
        if (listo) return;
        const { estado } = await MobileMesh.call(ayudante, "estado_pago", { referencia }).catch(() => ({ estado: "PENDIENTE" }));
        if (estado !== "PENDIENTE" || Date.now() > hasta) {
          listo = true;
          clearInterval(t);
          sub.remove();
          resolve({ referencia, estado });
        }
      };
      const t = setInterval(mirar, 4000);
      const sub = AppState.addEventListener("change", (e) => e === "active" && mirar());
    });
  },
  /** Base de datos cifrada en el móvil. */
  save: (collection, id, fields) => N.guardar(collection, id, fields),
  all: async (collection) => JSON.parse(await N.datos(collection)),
  delete: (collection, id) => N.borrar(collection, id),
  /** Colecciones públicas: lo que la app publica a propósito para que lo vean todos (ranking, catálogo). */
  publish: (collection, id, fields) => N.publicar(collection, id, fields),
  deletePublished: (collection, id) => N.borrarPublicado(collection, id),
  /** options: { donde: [["puntos", ">", 100]], orden: "puntos", desc: true, limite: 10 } */
  queryPublic: async (collection, options = {}) => JSON.parse(await N.consultarPublico(collection, JSON.stringify(options))),
  useRelay: (address) => N.usarRele(address ?? null),
  /** Ayudar a otros: este móvil hace de relé solo en wifi y con batería ≥15 % (encendido por defecto; `false` lo apaga). */
  actAsSupernode: (enabled) => N.hacerDeSupernodo(!!enabled),
  isSupernode: () => N.esSupernodo(),
  /** Avisos al instante con la pantalla apagada y sin Google (encendido por defecto; se pausa por debajo del 15 % de batería). */
  instantReception: (enabled) => N.recepcionInstantanea(!!enabled),
  diagnostics: () => N.diagnostico(),
};

/**
 * Para el campo donde se escribe el código en papel: mientras se teclea lo deja en grupos de 4
 * («2Y3Q-7KM9-…»), en mayúsculas, sin letras que no existen en el código y como mucho 56 letras.
 * Úsalo en `onChangeText={(t) => setCode(formatCode(t))}`.
 */
export function formatCode(texto) {
  const limpio = texto.toUpperCase().replace(/O/g, "0").replace(/[IL]/g, "1").replace(/[^0-9A-HJKMNP-TV-Z]/g, "").slice(0, 56);
  return limpio.replace(/(.{4})(?=.)/g, "$1-");
}

MobileMesh.formatCode = formatCode;

// Alias en español (obsoletos): siguen funcionando, avisan una vez por nombre y llaman al nombre inglés.
const alias = {
  estadoCuenta: "state",
  iniciar: "start",
  registrar: "register",
  entrar: "signIn",
  miUsuario: "myUsername",
  codigoPapel: "paperCode",
  entrarConCodigo: "recoverWithPaper",
  existeUsuario: "userExists",
  protegida: "isProtected",
  proteger: "protectWithGuardian",
  anadirPorNombre: "addByName",
  aceptarSolicitudes: "acceptRequests",
  actualizar: "refresh",
  usarGuiaDeNombres: "useNameGuide",
  invitacion: "invitation",
  anadirContacto: "addContact",
  contactos: "contacts",
  enviar: "sendMessage",
  mensajes: "messages",
  alRecibir: "onReceive",
  elegirArchivo: "pickFile",
  enviarArchivo: "sendFile",
  verArchivo: "viewFile",
  guardarEnDescargas: "saveToDownloads",
  llamar: "call",
  pagar: "pay",
  guardar: "save",
  datos: "all",
  borrar: "delete",
  publicar: "publish",
  borrarPublicado: "deletePublished",
  consultarPublico: "queryPublic",
  usarRele: "useRelay",
  hacerDeSupernodo: "actAsSupernode",
  esSupernodo: "isSupernode",
  recepcionInstantanea: "instantReception",
  diagnostico: "diagnostics",
  formatearCodigo: "formatCode",
};
for (const [viejo, nuevo] of Object.entries(alias)) {
  MobileMesh[viejo] = (...args) => {
    obsoleto(viejo, nuevo);
    return MobileMesh[nuevo](...args);
  };
}

export function formatearCodigo(texto) {
  return MobileMesh.formatearCodigo(texto);
}

export default MobileMesh;
