#!/usr/bin/env python3
"""Aviso opcional por Firebase (FCM) para el relé de MobileMesh.

Uso: mesh-rele 0.0.0.0:7900 --avisos tools/avisar-fcm.py
Necesita la variable FCM_CUENTA con la ruta del archivo JSON de la cuenta de servicio de tu proyecto
de Firebase (Configuración del proyecto → Cuentas de servicio → Generar clave privada).

El relé lo ejecuta con la ficha del teléfono como único argumento. Manda un mensaje de datos sin
contenido ({"mm": "1"}) con prioridad alta: solo despierta a la app para que recoja su buzón.
Solo usa la biblioteca estándar de Python y el programa `openssl` para firmar.
"""
import base64
import json
import os
import subprocess
import sys
import time
import urllib.request

CACHE = os.path.join(os.environ.get("TMPDIR", "/tmp"), "mobilemesh-fcm-token.json")


def b64(data: bytes) -> str:
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def access_token(account: dict) -> str:
    try:
        with open(CACHE) as f:
            c = json.load(f)
        if c["exp"] > time.time() + 60:
            return c["token"]
    except (OSError, ValueError, KeyError):
        pass
    now = int(time.time())
    head = b64(json.dumps({"alg": "RS256", "typ": "JWT"}).encode())
    claims = b64(json.dumps({
        "iss": account["client_email"],
        "scope": "https://www.googleapis.com/auth/firebase.messaging",
        "aud": account["token_uri"],
        "iat": now,
        "exp": now + 3600,
    }).encode())
    unsigned = f"{head}.{claims}".encode()
    key_file = CACHE + ".pem"
    fd = os.open(key_file, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        f.write(account["private_key"])
    try:
        sig = subprocess.run(["openssl", "dgst", "-sha256", "-sign", key_file], input=unsigned, capture_output=True, check=True).stdout
    finally:
        os.remove(key_file)
    body = f"grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer&assertion={unsigned.decode()}.{b64(sig)}"
    with urllib.request.urlopen(urllib.request.Request(account["token_uri"], body.encode()), timeout=15) as r:
        t = json.load(r)
    fd = os.open(CACHE, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        json.dump({"token": t["access_token"], "exp": now + int(t.get("expires_in", 3600))}, f)
    return t["access_token"]


def main() -> int:
    if len(sys.argv) != 2:
        print("uso: avisar-fcm.py <ficha del teléfono>", file=sys.stderr)
        return 2
    with open(os.environ["FCM_CUENTA"]) as f:
        account = json.load(f)
    msg = {"message": {"token": sys.argv[1], "data": {"mm": "1"}, "android": {"priority": "high"}}}
    req = urllib.request.Request(
        f"https://fcm.googleapis.com/v1/projects/{account['project_id']}/messages:send",
        json.dumps(msg).encode(),
        {"Authorization": f"Bearer {access_token(account)}", "Content-Type": "application/json"},
    )
    try:
        urllib.request.urlopen(req, timeout=15).read()
    except urllib.error.HTTPError as e:
        print(f"Firebase rechazó el aviso: {e.code} {e.read()[:300]!r}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
