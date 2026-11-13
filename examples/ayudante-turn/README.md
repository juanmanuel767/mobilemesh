# Ayudante de TURN (llamadas entre redes distintas)

Las llamadas de MobileMesh van directas entre los dos móviles. Cuando no hay camino directo (uno en
wifi y otro en datos, o un router que no deja), hace falta un **servidor TURN** que reenvíe el audio
—cifrado: el servidor no puede escucharlo—. Cloudflare los da con credenciales que caducan, y
pedirlas exige un token de API.

Ese token **no puede ir dentro de la app**: quien extrajera el APK generaría credenciales sin límite
contra la cuenta. Vive solo aquí.

```
App: llamar("turn", "credenciales")
  └─► ayudante «turn» (turn.py)  ──► Cloudflare Realtime TURN
  ◀── {iceServers: […], caduca: <ms>}  ── la app las guarda cifradas y las reusa hasta que caduquen
```

## Poner en marcha

1. En el panel de Cloudflare: **Realtime → TURN → Create**. Apunta el *TURN Key ID* y su *API token*.
2. `claves.env` fuera del repositorio (permisos 600):
   ```
   CLOUDFLARE_TURN_KEY_ID=…
   CLOUDFLARE_TURN_TOKEN=…
   TURN_HORAS=24
   ```
3. Cuenta del ayudante y su nombre en la guía:
   `mesh turn-mesh init` y `mesh turn-mesh registrar-nombre <guía> turn <ip:puerto>`
4. Arrancar: `MM_CLAVES=/ruta/claves.env mesh turn-mesh ayudante <ip:puerto> examples/ayudante-turn/turn.py`

## En la app

```js
const { iceServers, caduca } = await MobileMesh.llamar("turn", "credenciales");
new RTCPeerConnection({ iceServers });
```

Las credenciales son las mismas para todos mientras valgan: el ayudante las guarda en
`turn-credenciales.json` (junto a `claves.env`) y pide otras a Cloudflare cuando les queda menos de
una hora. Si el ayudante se queda sin internet devuelve solo STUN, con lo que las llamadas en la
misma wifi siguen funcionando.

Conviene que la app las pida al abrirse y las guarde: así tiene credenciales válidas aunque al
llamar no alcance al ayudante. Ejemplo completo: `turn.ts` de ChatMesh.
