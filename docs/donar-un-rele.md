# Cómo donar un relé (y un guardián)

Un relé conecta teléfonos de redes distintas; un guardián guarda copias cifradas para recuperar
cuentas. Ninguno de los dos puede leer nada. Cuantos más haya, más rápido y fiable es MobileMesh para
todas las apps.

```
Tu relé ──► el proyecto lo prueba ──► entra en el directorio firmado ──► todas las apps lo usan solas
```

## Opción A: un Android viejo (lo más fácil)

1. Instala la app **«Relé MobileMesh»** (`android/rele-movil`).
2. Ábrela y acepta: notificaciones y «no optimizar batería».
3. En **Ajustes → Batería → Optimización** de la app, pon en **Permitir**: Autoinicio, Inicio
   relacionado y Ejecución en segundo plano (nombres de nubia/ZTE; en otras marcas se parecen).
4. Déjalo **enchufado y en la wifi de casa**. La app abre sola los puertos 7900 y 7954 del router
   (UPnP) y nunca usa más de 500 KB/s.
5. En la pantalla de la app copia la línea **«invitación: mobilemesh:2:…»**.

¿Tu IP de casa cambia? Crea un nombre gratis en DuckDNS y escríbelo en el archivo
`ruta-publica.txt` de la app (por ejemplo `mirele.duckdns.org:7954`).

Para páginas `https://` el relé necesita candado: con el nombre de DuckDNS, el proyecto pide un
certificado gratis de Let's Encrypt y lo deja en el móvil con `tools/renovar-certificado.sh`
(cada mes). El relé lo usa solo, en el mismo puerto 7901.

## Opción B: un PC o servidor Linux

```sh
sudo sh tools/instalar-rele.sh ./mesh-rele 7900
```

Abre el puerto 7900 (TCP) en el router o en el panel del proveedor.

## Enviar la donación

Manda al proyecto:

- la dirección del relé: `tu-ip-o-nombre:7900`;
- la invitación del guardián (si usas la app).

## Lo que hace el proyecto

1. Comprueba que responde: `printf 'QUIEN\n' | nc tu-ip 7900` debe contestar una IP.
2. Lo añade y sube la versión del directorio:

```sh
sh tools/publicar-directorio.sh <versión> "relé1:7900,tu-ip:7900" "invitación1,tu-invitación"
```

Las apps lo descargan solas al abrirse; no hace falta publicar otra versión.

## Requisitos

| | Mínimo |
|---|---|
| Internet | Wifi de casa (no datos móviles: desde fuera no se puede entrar) |
| Router | Con UPnP, o abrir a mano los puertos 7900 y 7954 |
| Equipo | Android 9 o superior (64 bits), o Linux |
| Corriente | Siempre enchufado |

## Lo que tu relé SÍ ve

Direcciones IP, horas y cantidad de datos. No ve mensajes ni archivos: van cifrados de extremo a
extremo.
