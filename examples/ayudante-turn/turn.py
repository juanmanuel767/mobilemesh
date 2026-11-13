#!/usr/bin/env python3
"""Ayudante de TURN para MobileMesh: credenciales temporales para las llamadas.

Una llamada entre dos móviles en redes distintas (uno en wifi, otro en datos)
necesita un servidor TURN que reenvíe el audio cifrado. Cloudflare los da con
credenciales temporales, y para pedirlas hace falta un token de API.

Ese token NO puede ir dentro de la app: quien extrajera el APK podría generar
credenciales sin límite contra la cuenta. Vive aquí, en el ayudante, que sabe
con certeza qué cuenta le llama; la app solo recibe un usuario y una contraseña
que caducan en unas horas.

Uso:
    MM_CLAVES=/ruta/claves.env  mesh <carpeta> ayudante <ip:puerto> /ruta/turn.py

claves.env (fuera de git, permisos 600):
    CLOUDFLARE_TURN_KEY_ID=<id de la TURN key>
    CLOUDFLARE_TURN_TOKEN=<token de API de esa TURN key>
    TURN_HORAS=24          # opcional: cuánto duran las credenciales (por defecto 24)

Funciones (desde la app: llamar("turn", <función>, datos)):
    credenciales {}  → {iceServers: [...], caduca: <milisegundos epoch>, horas: <n>}
"""
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

API = "https://rtc.live.cloudflare.com/v1/turn/keys/{}/credentials/generate-ice-servers"

# Unas credenciales a punto de caducar no sirven para una llamada que empieza
# ahora: se piden otras cuando les queda menos de esto.
MARGEN_S = 3600

# Si Cloudflare no responde, una llamada en la misma wifi sigue funcionando solo
# con STUN. Mejor eso que un error.
SOLO_STUN = [{"urls": ["stun:stun.cloudflare.com:3478", "stun:stun.cloudflare.com:53"]}]


def claves():
    c = {}
    for linea in Path(os.environ["MM_CLAVES"]).read_text().splitlines():
        if "=" in linea and not linea.lstrip().startswith("#"):
            k, v = linea.split("=", 1)
            c[k.strip()] = v.strip()
    return c


# Las mismas credenciales valen para todo el que llame mientras no caduquen: se
# guardan aquí para no pedirle unas nuevas a Cloudflare en cada llamada.
CACHE = Path(os.environ["MM_CLAVES"]).with_name("turn-credenciales.json")


def guardadas():
    if not CACHE.exists():
        return None
    try:
        c = json.loads(CACHE.read_text())
    except ValueError:
        return None
    if c.get("caduca", 0) - time.time() * 1000 < MARGEN_S * 1000:
        return None
    return c


def generar(c):
    horas = max(1, int(c.get("TURN_HORAS", "24")))
    ttl = horas * 3600
    peticion = urllib.request.Request(
        API.format(c["CLOUDFLARE_TURN_KEY_ID"]),
        data=json.dumps({"ttl": ttl}).encode(),
        headers={
            "Authorization": f"Bearer {c['CLOUDFLARE_TURN_TOKEN']}",
            "Content-Type": "application/json",
            # Sin esto Cloudflare responde 403 (error 1010): bloquea el nombre
            # que pone Python por defecto («Python-urllib»).
            "User-Agent": "mobilemesh-ayudante-turn",
        },
        method="POST",
    )
    with urllib.request.urlopen(peticion, timeout=15) as r:
        cuerpo = json.load(r)
    # Cloudflare devuelve un solo objeto con todas las direcciones; la app
    # espera una lista, como la que pide RTCPeerConnection.
    ice = cuerpo.get("iceServers")
    if isinstance(ice, dict):
        ice = [ice]
    if not ice:
        raise ValueError("Cloudflare no devolvió servidores")
    resultado = {"iceServers": ice, "caduca": int((time.time() + ttl) * 1000), "horas": horas}
    CACHE.write_text(json.dumps(resultado, indent=1))
    CACHE.chmod(0o600)
    return resultado


def main():
    funcion = sys.argv[1]
    json.load(sys.stdin)  # {"de": cuenta, "datos": {…}}: aquí no hace falta nada de la app

    if funcion == "credenciales":
        c = claves()
        falta = [k for k in ("CLOUDFLARE_TURN_KEY_ID", "CLOUDFLARE_TURN_TOKEN") if not c.get(k)]
        if falta:
            raise ValueError(f"faltan en claves.env: {', '.join(falta)}")
        previas = guardadas()
        if previas:
            return previas
        try:
            return generar(c)
        except urllib.error.HTTPError as e:
            raise ValueError(f"Cloudflare rechazó la petición ({e.code}): revisa la TURN key y su token")
        except OSError as e:
            # Sin internet en el ayudante: la app sigue pudiendo llamar en la
            # misma wifi, así que se le devuelve STUN en vez de un error.
            return {"iceServers": SOLO_STUN, "caduca": 0, "horas": 0, "aviso": f"sin TURN: {e}"}

    raise ValueError(f"no existe la función {funcion}")


if __name__ == "__main__":
    try:
        print(json.dumps(main(), ensure_ascii=False))
    except Exception as e:  # el mensaje vuelve a la app como error de `llamar`
        print(str(e), file=sys.stderr)
        raise SystemExit(1)
