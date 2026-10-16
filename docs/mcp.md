# MobileMesh para IA (servidor MCP)

`mesh-mcp` deja que cualquier IA compatible con MCP (Claude, ChatGPT, Codex…) use MobileMesh:
leer la guía del SDK, crear nodos de prueba, guardar y consultar datos, añadir contactos, dar
permisos y comprobar la red.

## Instalar

```sh
cd core && cargo build --release --bin mesh-mcp
claude mcp add mobilemesh -- "$(pwd)/target/release/mesh-mcp"
```

## Herramientas

| Herramienta | Qué hace |
|---|---|
| `guia` | Guía completa del SDK de Android (Kotlin) |
| `crear_nodo` | Nodo de prueba (identidad + base de datos cifrada) en una carpeta |
| `guardar_dato` / `ver_datos` | Escribir y leer registros de una colección |
| `invitacion` / `anadir_contacto` | Conectar nodos entre sí |
| `dar_permiso` | Compartir una colección: leer, escribir o nada |
| `estado_red` | Relés y guardianes del directorio firmado y si responden |

## Seguridad

- Los nodos viven en carpetas locales; solo `estado_red` sale a internet (a los relés y guardianes públicos).
- No hay herramientas para publicar el directorio ni para borrar cuentas: eso queda en manos de una persona.
