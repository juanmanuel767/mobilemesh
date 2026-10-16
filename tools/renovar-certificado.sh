#!/bin/sh
# Renueva (si toca) el certificado gratis de Let's Encrypt del relé y lo deja en el móvil del relé.
# El relé lo vuelve a leer en cada conexión: no hace falta reiniciarlo. Ejecutar cada mes con el
# móvil conectado (por cable o depuración inalámbrica). La primera vez se pidió con:
#   DuckDNS_Token=<token> ~/.acme.sh/acme.sh --issue --server letsencrypt --dns dns_duckdns -d <nombre>.duckdns.org
set -eu
DOMINIO="${1:-mobilemeshjuan.duckdns.org}"
SERIE="${2:-323952058590}"
D="$HOME/.acme.sh/${DOMINIO}_ecc"
"$HOME/.acme.sh/acme.sh" --renew -d "$DOMINIO" --ecc >/dev/null 2>&1 || true  # «aún no toca» no es error
adb -s "$SERIE" shell "run-as org.mobilemesh.rele sh -c 'cat > files/cert.pem'" < "$D/fullchain.cer"
adb -s "$SERIE" shell "run-as org.mobilemesh.rele sh -c 'cat > files/clave.pem && chmod 600 files/clave.pem'" < "$D/$DOMINIO.key"
echo "Certificado de $DOMINIO en el móvil (válido hasta $(openssl x509 -enddate -noout -in "$D/fullchain.cer" | cut -d= -f2))."
