# MobileMesh para Flutter

Base de datos y mensajería cifrada entre dispositivos, sin servidores. Cada usuario guarda sus
datos cifrados en su teléfono y los comparte cifrado de extremo a extremo con sus contactos.

Este plugin habla por `MethodChannel` con el SDK de Android (Kotlin). **Hoy solo funciona en
Android**; el SDK de Swift para iOS todavía está pendiente.

## Añadirlo a una app

En el `pubspec.yaml` de tu app:

```yaml
dependencies:
  mobilemesh:
    path: ../mobilemesh/flutter   # o la URL de git del repositorio
```

La app debe tener `compileSdk` 35 o superior, `minSdk` 28 y Kotlin 2.2.0 o superior. La marca de la
app debe ir en `android/app/src/main/assets/mobilemesh.properties` (`appId=mm-<32 hex>`); guárdala y
no la borres. Si falta, se usa el nombre del paquete. En este repositorio,
`tools/mobilemesh.gradle.kts` la genera sola al compilar.

## Ejemplo mínimo

```dart
import 'package:mobilemesh/mobilemesh.dart';

final mesh = await MobileMesh.start();
final notas = mesh.collection('notas');

await notas.save('n1', {'texto': 'hola'});
final documentos = await notas.all();

print(documentos.first.fields['texto']); // hola
```

`start()` abre la cuenta (la crea la primera vez) y devuelve su identificador público con
`accountId()`. El parámetro opcional `appId` se acepta por compatibilidad con la versión web, pero
en Android se ignora: la marca sale de `assets/mobilemesh.properties` o del nombre del paquete.

## Qué incluye este plugin

`start`, `accountId`, `save`, `all`, `query` (`where`/`sortBy`/`limit`/`get`), `invitation`,
`addContact`, `contacts`, `sendMessage`, `messages`, `shareWith` y `syncWith`.

`syncWith` sincroniza todo lo compartido con un contacto; en el SDK de Kotlin vive en la colección,
pero internamente solo usa el núcleo. Aún no se exponen archivos, colecciones públicas, recepción en
vivo ni recuperación de cuenta.

En `example/lib/main.dart` hay una app mínima que abre la cuenta, guarda una nota y la muestra. Para
ejecutarla, genera las carpetas de plataforma con `flutter create .` dentro de `example/`.
