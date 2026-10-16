// MobileMesh para React Native (Android): base de datos, mensajes, archivos y avisos sin servidores.
import { NativeEventEmitter, NativeModules } from "react-native";

const N = NativeModules.MobileMesh;
if (!N) throw new Error("MobileMesh: módulo nativo no encontrado. ¿Recompilaste la app tras instalarlo? (solo Android)");
const eventos = new NativeEventEmitter(N);

export const MobileMesh = {
  /** "sin cuenta" | "bloqueado" | "lista" */
  estadoCuenta: () => N.estadoCuenta(),
  /** Abre (o crea la primera vez) la cuenta de este móvil y pide los permisos de avisos. Devuelve su id. */
  iniciar: () => N.iniciar(),

  // Usuarios por nombre, como en cualquier base de datos.
  /** Tras `iniciar`: registra el usuario (y guarda la cuenta en el guardián). Responde un aviso o null. */
  registrar: (usuario, contrasena) => N.registrar(usuario, contrasena),
  /** En un móvil nuevo, en lugar de `iniciar`: recupera la cuenta con usuario y contraseña. */
  entrar: (usuario, contrasena) => N.entrar(usuario, contrasena),
  miUsuario: () => N.miUsuario(),
  /** Código en papel (56 letras): enséñalo para apuntarlo; recupera la cuenta sin contraseña. */
  codigoPapel: () => N.codigoPapel(),
  /** Contraseña olvidada: entra con el código en papel y elige una contraseña nueva. */
  entrarConCodigo: (usuario, codigo, nuevaContrasena) => N.entrarConCodigo(usuario, codigo, nuevaContrasena),
  existeUsuario: (usuario) => N.existeUsuario(usuario),
  /** ¿Se puede recuperar la cuenta al formatear? (tiene copia en el guardián, que se actualiza sola) */
  protegida: () => N.protegida(),
  /** Protege la cuenta si al registrarse no se pudo. */
  proteger: (contrasena) => N.proteger(contrasena),
  /** Añade a alguien por su nombre de usuario; su app te añadirá sola. */
  anadirPorNombre: (usuario) => N.anadirPorNombre(usuario),
  /** Acepta a quienes quieren hablar contigo. Devuelve sus nombres. */
  aceptarSolicitudes: () => N.aceptarSolicitudes(),
  /** Actualiza tu invitación en la guía y acepta solicitudes. Llámalo al abrir la app y cada poco. */
  actualizar: () => N.actualizar(),
  usarGuiaDeNombres: (invitacion) => N.usarGuiaDeNombres(invitacion ?? null),

  /** Texto para que otro te añada (compártelo o ponlo en un QR). */
  invitacion: () => N.invitacion(),
  anadirContacto: (nombre, invitacion) => N.anadirContacto(nombre, invitacion),
  contactos: () => N.contactos(),
  /** Envía un texto. Responde «entregado» o «en buzón». */
  enviar: (contacto, texto) => N.enviar(contacto, texto),
  /** [{id, contacto, enviado, texto, estado, progreso, archivo, tipo, tamano, disponible}] */
  mensajes: async () => JSON.parse(await N.mensajes()),
  /** Avisa con `alNuevo(n)` cada vez que llega algo. Devuelve la función para dejar de escuchar. */
  alRecibir: (alNuevo) => {
    const s = eventos.addListener("MobileMeshNuevos", alNuevo);
    return () => s.remove();
  },
  /** Selector de archivos de Android: {uri, nombre} o null. */
  elegirArchivo: () => N.elegirArchivo(),
  enviarArchivo: (contacto, uri, nombre) => N.enviarArchivo(contacto, uri, nombre),
  /** Ruta file:// descifrada en la caché (para <Image>). */
  verArchivo: (id) => N.verArchivo(id),
  guardarEnDescargas: (id) => N.guardarEnDescargas(id),
  /**
   * Llama a una función de servidor de un ayudante (añádelo antes con `anadirPorNombre`). El ayudante
   * sabe con certeza qué cuenta llama. Devuelve su resultado (objeto si es JSON).
   * Ejemplo de pagos con Wompi: examples/ayudante-wompi.
   */
  llamar: async (ayudante, funcion, datos = {}) => {
    const r = await N.llamar(ayudante, funcion, JSON.stringify(datos));
    try {
      return JSON.parse(r);
    } catch {
      return r;
    }
  },
  /** Base de datos cifrada en el móvil. */
  guardar: (coleccion, id, campos) => N.guardar(coleccion, id, campos),
  datos: async (coleccion) => JSON.parse(await N.datos(coleccion)),
  borrar: (coleccion, id) => N.borrar(coleccion, id),
  usarRele: (direccion) => N.usarRele(direccion ?? null),
  diagnostico: () => N.diagnostico(),
};

/**
 * Para el campo donde se escribe el código en papel: mientras se teclea lo deja en grupos de 4
 * («2Y3Q-7KM9-…»), en mayúsculas, sin letras que no existen en el código y como mucho 56 letras.
 * Úsalo en `onChangeText={(t) => setCodigo(formatearCodigo(t))}`.
 */
export function formatearCodigo(texto) {
  const limpio = texto.toUpperCase().replace(/O/g, "0").replace(/[IL]/g, "1").replace(/[^0-9A-HJKMNP-TV-Z]/g, "").slice(0, 56);
  return limpio.replace(/(.{4})(?=.)/g, "$1-");
}

MobileMesh.formatearCodigo = formatearCodigo;

export default MobileMesh;
