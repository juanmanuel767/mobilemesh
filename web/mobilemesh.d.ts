// Tipos TypeScript del SDK web de MobileMesh (web/mobilemesh.js).
// Escritos a mano según lo que el código y el núcleo WebAssembly producen de verdad.

/** Operadores válidos de una consulta. */
export type QueryOperator =
  | "=="
  | "!="
  | "<"
  | "<="
  | ">"
  | ">="
  | "contiene"
  | "empieza"
  | "existe";

/** Documento tal como lo devuelve el núcleo: los campos van en `campos`. */
export interface Document {
  id: string;
  campos: Record<string, string>;
}

/** Mensaje del historial: `[{"contacto", "saliente", "texto", "estado"}]`. */
export interface Message {
  contacto: string;
  /** `true` si lo enviaste tú, `false` si lo recibiste. */
  saliente: boolean;
  texto: string;
  estado: string;
}

/** Resultado de `sendMessage`: «entregado», «en buzón» o «pendiente: …». */
export type DeliveryStatus = "entregado" | "en buzón" | `pendiente: ${string}`;

/** Permisos de una colección compartida (se aceptan los nombres en inglés y en español). */
export type Permission = "read" | "write" | "none" | "leer" | "escribir" | "nada";

/** Consulta encadenable que empieza con `collection(name).where(...)`. */
export interface Query {
  /** Añade otro filtro. */
  where(field: string, op: QueryOperator, value: unknown): Query;
  /** Ordena por un campo; `descending` es `false` por defecto. */
  sortBy(field: string, descending?: boolean): Query;
  /** Limita el número de documentos. */
  limit(n: number): Query;
  /** Ejecuta la consulta y devuelve los documentos. */
  get(): Promise<Document[]>;
}

/** Colección devuelta por `mesh.collection(name)`. */
export interface Collection {
  /** Guarda todos los campos de un objeto en un registro (los valores se convierten a texto). */
  save(id: string, fields: Record<string, unknown>): Promise<void>;
  /** Registros de la colección. */
  all(): Promise<Document[]>;
  /** Empieza una consulta con un filtro. */
  where(field: string, op: QueryOperator, value: unknown): Query;
}

/** Opciones de arranque. */
export interface StartOptions {
  /** Marca de la aplicación; si se omite se usa el dominio de la página. */
  appId?: string;
}

/**
 * Cuenta de MobileMesh en esta página.
 * Ojo: usa `MobileMesh.start()`; no construyas la clase a mano.
 */
export class MobileMesh {
  /** Arranca MobileMesh: carga el núcleo en un trabajador y conecta la red. */
  static start(options?: StartOptions): Promise<MobileMesh>;
  /** @deprecated Usa `MobileMesh.start()`. */
  static iniciar(options?: StartOptions): Promise<MobileMesh>;

  private constructor();

  /** Identificador de la cuenta de este navegador (se crea la primera vez y se guarda). */
  account(): Promise<string>;
  /** @deprecated Usa `account()`. */
  cuenta(): Promise<string>;

  /** Guarda un campo de un registro de una colección (cifrado y permanente). */
  save(
    collection: string,
    record: string,
    field: string,
    value: unknown
  ): Promise<void>;
  /** @deprecated Usa `save(collection, record, field, value)`. */
  guardar(
    collection: string,
    record: string,
    field: string,
    value: unknown
  ): Promise<void>;

  /** Guarda un valor de cualquier tamaño (por dentro se parte si hace falta). */
  saveLarge(
    collection: string,
    record: string,
    field: string,
    value: unknown
  ): Promise<void>;

  /** Lee un valor guardado con `saveLarge()`, ya unido. Cadena vacía si no está. */
  readLarge(
    collection: string,
    record: string,
    field: string
  ): Promise<string>;

  /** Abre una colección con `save`, `all` y `where` encadenable. */
  collection(name: string): Collection;

  /** Registros de una colección: `[{ id, campos }]`. */
  all(collection: string): Promise<Document[]>;
  /** @deprecated Usa `all(collection)`. */
  datos(collection: string): Promise<Document[]>;

  /** Mi invitación (para que otro me añada como contacto). */
  invitation(): Promise<string>;
  /** @deprecated Usa `invitation()`. */
  invitacion(): Promise<string>;

  /** Añade un contacto con un nombre, a partir de la invitación que compartió. */
  addContact(name: string, invitation: string): Promise<void>;
  /** @deprecated Usa `addContact(name, invitation)`. */
  anadirContacto(name: string, invitation: string): Promise<void>;

  /** Envía un texto y devuelve su estado de entrega. */
  sendMessage(name: string, text: string): Promise<DeliveryStatus>;
  /** @deprecated Usa `sendMessage(name, text)`. */
  enviar(name: string, text: string): Promise<DeliveryStatus>;

  /** Todos los mensajes del historial. */
  messages(): Promise<Message[]>;
  /** @deprecated Usa `messages()`. */
  mensajes(): Promise<Message[]>;

  /** Empieza a recibir mientras la página esté abierta; `callback(n)` avisa de los nuevos. */
  onReceive(callback: (n: number) => void): void;
  /** @deprecated Usa `onReceive(callback)`. */
  recibir(callback: (n: number) => void): void;

  /** Comparte una colección con un contacto: `"read"`, `"write"` o `"none"` (quitar). */
  shareWith(
    collection: string,
    contact: string,
    permission: Permission
  ): Promise<void>;
  /** @deprecated Usa `shareWith(collection, contact, permission)`. */
  darPermiso(
    collection: string,
    contact: string,
    permission: Permission
  ): Promise<void>;

  /** Sincroniza con un contacto las colecciones compartidas. */
  syncWith(contact: string): Promise<string>;
  /** @deprecated Usa `syncWith(contact)`. */
  sincronizar(contact: string): Promise<string>;

  /** Mi IP pública según un relé («host:puerto»). */
  publicAddress(relay: string): Promise<string>;
  /** @deprecated Usa `publicAddress(relay)`. */
  quien(relay: string): Promise<string>;

  /**
   * Directorio firmado de relés y guardianes que sirve un relé (firma comprobada).
   * La forma exacta del objeto no está documentada: se devuelve como `unknown`.
   */
  directory(relay: string): Promise<unknown>;
  /** @deprecated Usa `directory(relay)`. */
  directorio(relay: string): Promise<unknown>;
}
