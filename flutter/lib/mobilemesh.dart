import 'package:flutter/services.dart';

/// Base de datos y mensajería cifrada entre dispositivos, sin servidores.
///
/// Cada usuario guarda sus datos cifrados en su teléfono y los comparte cifrados de extremo a
/// extremo. Por ahora este plugin solo funciona en Android; iOS espera su SDK en Swift.
class MobileMesh {
  MobileMesh._(this._id);

  static const MethodChannel _channel = MethodChannel('mobilemesh');
  final String _id;

  /// Abre la cuenta de este teléfono (la crea la primera vez) y devuelve la instancia lista.
  ///
  /// En Android el identificador de la app sale de `assets/mobilemesh.properties` (o del nombre del
  /// paquete), así que [appId] se acepta por compatibilidad con la versión web pero no se usa aquí.
  static Future<MobileMesh> start({String? appId}) async {
    final id = await _channel.invokeMethod<String>('start');
    return MobileMesh._(id ?? '');
  }

  /// Identificador público de la cuenta.
  Future<String> accountId() async {
    final id = await _channel.invokeMethod<String>('accountId');
    return id ?? _id;
  }

  /// Abre (o crea) una colección de documentos cifrados.
  MobileMeshCollection collection(String name) => MobileMeshCollection._(name);

  /// Invitación de este teléfono: texto para compartir o poner en un QR.
  Future<String> invitation() async {
    return await _channel.invokeMethod<String>('invitation') ?? '';
  }

  /// Añade un contacto con su invitación. Devuelve `true` si era otro dispositivo de tu cuenta.
  Future<bool> addContact(String name, String invitation) async {
    return await _channel.invokeMethod<bool>('addContact', {
          'name': name,
          'invitation': invitation,
        }) ??
        false;
  }

  /// Nombres de tus contactos.
  Future<List<String>> contacts() async {
    final lista = await _channel.invokeMethod<List<Object?>>('contacts') ?? const [];
    return lista.map((e) => e.toString()).toList();
  }

  /// Envía un mensaje de texto. Devuelve «entregado» o «en buzón».
  Future<String> sendMessage(String name, String text) async {
    return await _channel.invokeMethod<String>('sendMessage', {
          'contact': name,
          'text': text,
        }) ??
        '';
  }

  /// Todos los mensajes, del más antiguo al más reciente.
  Future<List<MeshMessage>> messages() async {
    final lista = await _channel.invokeMethod<List<Object?>>('messages') ?? const [];
    return lista.map((e) => MeshMessage.fromMap(_mapa(e))).toList();
  }

  /// Comparte una colección con un contacto, con lectura o con escritura.
  Future<void> shareWith(String collection, String name, bool canWrite) async {
    await _channel.invokeMethod<void>('shareWith', {
      'collection': collection,
      'contact': name,
      'canWrite': canWrite,
    });
  }

  /// Sincroniza ahora con un contacto todo lo compartido (también ocurre solo cada pocos segundos).
  Future<String> syncWith(String name) async {
    return await _channel.invokeMethod<String>('syncWith', {'contact': name}) ?? '';
  }
}

/// Colección de documentos de MobileMesh.
class MobileMeshCollection {
  MobileMeshCollection._(this.name);

  /// Nombre de la colección.
  final String name;

  /// Guarda (o actualiza) los campos de un documento con ese [id].
  Future<void> save(String id, Map<String, String> fields) async {
    await MobileMesh._channel.invokeMethod<void>('save', {
      'collection': name,
      'id': id,
      'fields': fields,
    });
  }

  /// Todos los documentos de la colección.
  Future<List<MeshDocument>> all() async {
    final lista = await MobileMesh._channel.invokeMethod<List<Object?>>(
          'all',
          {'collection': name},
        ) ??
        const [];
    return lista.map((e) => MeshDocument.fromMap(_mapa(e))).toList();
  }

  /// Empieza una consulta con un filtro. Operadores: `==`, `!=`, `<`, `<=`, `>`, `>=`,
  /// `contiene`, `empieza` y `existe`.
  MobileMeshQuery where(String field, String op, [Object? value]) =>
      MobileMeshQuery._(name).where(field, op, value);
}

/// Consulta encadenable sobre una colección.
class MobileMeshQuery {
  MobileMeshQuery._(this._collection);

  final String _collection;
  final List<List<Object?>> _where = [];
  String? _order;
  bool _descending = false;
  int? _limit;

  /// Añade otro filtro a la consulta.
  MobileMeshQuery where(String field, String op, [Object? value]) {
    _where.add([field, op, value]);
    return this;
  }

  /// Ordena el resultado por [field], de forma ascendente o descendente.
  MobileMeshQuery sortBy(String field, {bool descending = false}) {
    _order = field;
    _descending = descending;
    return this;
  }

  /// Limita el número de documentos devueltos.
  MobileMeshQuery limit(int n) {
    _limit = n;
    return this;
  }

  /// Ejecuta la consulta y devuelve los documentos.
  Future<List<MeshDocument>> get() async {
    final lista = await MobileMesh._channel.invokeMethod<List<Object?>>('query', {
          'collection': _collection,
          'options': {
            'where': _where,
            'order': _order,
            'desc': _descending,
            'limit': _limit,
          },
        }) ??
        const [];
    return lista.map((e) => MeshDocument.fromMap(_mapa(e))).toList();
  }
}

/// Documento guardado en una colección.
class MeshDocument {
  const MeshDocument({required this.id, required this.fields});

  /// Identificador del documento.
  final String id;

  /// Campos guardados (texto, números, booleanos…).
  final Map<String, Object?> fields;

  /// Crea un documento a partir de la respuesta del canal nativo.
  factory MeshDocument.fromMap(Map<String, Object?> mapa) => MeshDocument(
        id: (mapa['id'] ?? '').toString(),
        fields: _mapa(mapa['fields']),
      );

  @override
  String toString() => 'MeshDocument($id, $fields)';
}

/// Mensaje enviado o recibido.
class MeshMessage {
  const MeshMessage({
    required this.id,
    required this.contact,
    required this.sent,
    required this.text,
    required this.status,
    this.file,
  });

  /// Identificador del mensaje.
  final String id;

  /// Nombre del contacto.
  final String contact;

  /// `true` si lo enviaste tú.
  final bool sent;

  /// Texto del mensaje.
  final String text;

  /// Estado del mensaje.
  final String status;

  /// Nombre del archivo adjunto, si lo hay.
  final String? file;

  /// Crea un mensaje a partir de la respuesta del canal nativo.
  factory MeshMessage.fromMap(Map<String, Object?> mapa) => MeshMessage(
        id: (mapa['id'] ?? '').toString(),
        contact: (mapa['contact'] ?? '').toString(),
        sent: mapa['sent'] as bool? ?? false,
        text: (mapa['text'] ?? '').toString(),
        status: (mapa['status'] ?? '').toString(),
        file: mapa['file']?.toString(),
      );

  @override
  String toString() => 'MeshMessage($id, $contact, $text)';
}

/// Convierte un mapa recibido del canal nativo a claves de texto.
Map<String, Object?> _mapa(Object? valor) {
  if (valor is Map) {
    return <String, Object?>{
      for (final entrada in valor.entries) entrada.key.toString(): entrada.value,
    };
  }
  return const <String, Object?>{};
}
