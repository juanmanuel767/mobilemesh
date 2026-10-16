# MobileMesh

Backend de código abierto donde **los propios dispositivos guardan sus datos** y se comunican directamente y cifrados, sin base de datos central obligatoria. Pensado para usarse **en lugar de Firebase o Supabase**.

Creado por **Juan Manuel Peralta Chacon** ([@juanmanuel767](https://github.com/juanmanuel767)). Licencia [MIT](LICENSE).

**¿Eres programador?**
- Android: [guía del SDK](docs/sdk.md) y el ejemplo [`examples/notas`](examples/notas).
- Web: [guía del SDK web](docs/sdk-web.md) (WebAssembly, funciona en páginas `https://`).
- IA (Claude, ChatGPT…): [servidor MCP](docs/mcp.md).
- ¿Quieres ayudar? [Dona un relé](docs/donar-un-rele.md) con un móvil viejo.

```
📱 Apps Android ─┐                         ┌─ Guardián: recuperar cuenta y datos
                 ├─► Relés (directorio firmado) ─┤
🌐 Apps web ─────┘   cifrado de extremo a extremo └─ Nadie puede leer los datos
```

- **Relés universales, guardián, directorio, candado y web:** [0012 guardián](docs/decisiones/0012-guardian.md) · [0013 directorio](docs/decisiones/0013-directorio.md) · [0014 auditoría de las piezas nuevas](docs/decisiones/0014-auditoria-piezas-nuevas.md)
- Reglas y amenazas: [docs/fase-0.md](docs/fase-0.md)
- Tecnologías elegidas: [docs/decisiones/0001-tecnologias.md](docs/decisiones/0001-tecnologias.md)
- **Auditoría y lanzamiento (Fase 13):** [docs/decisiones/0011-auditoria.md](docs/decisiones/0011-auditoria.md) · [Protocolo](docs/protocolo.md) · [Dependencias y licencias](docs/dependencias.md)
- SDK para desarrolladores (Fase 12): [docs/decisiones/0010-sdk.md](docs/decisiones/0010-sdk.md)
- Recibir con la app cerrada y avisos (Fase 11, en curso): [docs/decisiones/0009-avisos-y-segundo-plano.md](docs/decisiones/0009-avisos-y-segundo-plano.md)
- Permisos (Fase 10): [docs/decisiones/0008-permisos.md](docs/decisiones/0008-permisos.md)
- Relé, redes distintas y metadatos (Fase 9): [docs/decisiones/0007-rele-y-metadatos.md](docs/decisiones/0007-rele-y-metadatos.md)
- Copias cifradas en otros dispositivos (Fase 8): [docs/decisiones/0006-copias-en-otros-dispositivos.md](docs/decisiones/0006-copias-en-otros-dispositivos.md)
- Sin conexión y sincronización (Fase 7): [docs/decisiones/0005-sincronizacion.md](docs/decisiones/0005-sincronizacion.md)
- Fotos, vídeos y documentos (Fase 6): [docs/decisiones/0004-archivos.md](docs/decisiones/0004-archivos.md)
- Cifrado de extremo a extremo (Fase 5): [docs/decisiones/0003-cifrado-de-extremo-a-extremo.md](docs/decisiones/0003-cifrado-de-extremo-a-extremo.md)
- Identidad, cuentas, vinculación y recuperación (Fase 2): [docs/decisiones/0002-identidad-y-recuperacion.md](docs/decisiones/0002-identidad-y-recuperacion.md)

## Estado

| Hecho | Pendiente |
|---|---|
| **Probado entre dos teléfonos reales (TECNO KJ5 Android 13 y nubia Z2465N Android 16) por wifi, sin servidor.** Cuentas con varios dispositivos vinculados por QR, identidad protegida con Android Keystore, copia de recuperación cifrada con contraseña, contactos por invitación QR, base de datos local con migraciones, cifrado de extremo a extremo por mensaje (Double Ratchet) dentro de una conexión directa cifrada y autenticada, base de datos cifrada, envío de fotos, vídeos y documentos cifrados que continúa tras un corte, datos sincronizados entre los dispositivos de la cuenta con modo sin conexión y conflictos a la vista, reintentos automáticos, copias de seguridad cifradas guardadas por contactos o dispositivos propios que ellos no pueden leer, con comprobación sin descarga, recuperación tras perder el teléfono y borrado, relé autoalojable (`mesh-rele`) para redes distintas que no puede leer nada, mensajes rellenados a tamaño fijo, retirar dispositivos perdidos con una lista firmada, bloquear contactos, límites contra el spam, listas compartidas con permiso de lectura o escritura, servicio en segundo plano con avisos sin contenido, librería para desarrolladores (`mobilemesh-sdk`) con app de ejemplo, confirmación de guardado, cola de pendientes sin duplicados, rechazo de desconocidos y de impostores | Revocar dispositivos, entregar a todos los dispositivos de una cuenta, recuperación asistida, recibir con la app en segundo plano (Fase 11), relé permanente en un servidor propio, perforación de NAT, copiar los mensajes entre los dispositivos de la cuenta, réplicas, avisos push, SDK |

## Probar

```sh
cd core
cargo test                      # 68 pruebas, entre ellas 9 ataques simulados (cargo test --release --test medidas -- --nocapture para las mediciones)
cargo build
M=target/debug/mesh
$M /tmp/ana init && $M /tmp/beto init
$M /tmp/ana contacto beto $($M /tmp/beto id)
$M /tmp/beto contacto ana $($M /tmp/ana id)
$M /tmp/beto escuchar 127.0.0.1:7801 &          # otra terminal o en segundo plano
$M /tmp/ana enviar beto 127.0.0.1:7801 "hola"
$M /tmp/beto mensajes
```

## Relé

```sh
core/target/debug/mesh-rele 0.0.0.0:7900     # en cualquier equipo con dirección alcanzable
```

En la app: «Relé para otras redes» → `ip:puerto` → «Usar este relé» y reiniciarla. La invitación incluye entonces la ruta por el relé.

## App de Android

```sh
android/build.sh            # compila el núcleo, la librería, la app de pruebas y la app de ejemplo
android/build.sh instalar   # además lo instala y lo abre en el teléfono conectado por cable
```

La app recibe en el puerto 7801. Para hablar con el PC: en el PC `mesh <carpeta> escuchar 0.0.0.0:7802`, y en la app pegar la invitación que da `mesh <carpeta> invitacion <ip-del-pc>:7802`.
