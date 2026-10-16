#!/bin/sh
# Pide al router de casa (por UPnP, sin contraseña) que abra el puerto del relé hacia el móvil que lo
# ejecuta. La app «Relé MobileMesh» ya lo hace sola; esto es para hacerlo a mano desde un PC.
#
#   sh tools/abrir-puerto-upnp.sh 192.168.20.3 [puerto]      # abrir
#   sh tools/abrir-puerto-upnp.sh --cerrar [puerto]          # cerrar
set -eu
ROUTER="${ROUTER:-http://192.168.20.1:80/WANIPConnection}"
soap() {
    curl -s -m 8 "$ROUTER" -H 'Content-Type: text/xml; charset="utf-8"' \
        -H "SOAPAction: \"urn:schemas-upnp-org:service:WANIPConnection:1#$1\"" \
        -d "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:$1 xmlns:u=\"urn:schemas-upnp-org:service:WANIPConnection:1\">$2</u:$1></s:Body></s:Envelope>"
}
if [ "${1:-}" = --cerrar ]; then
    PUERTO="${2:-7900}"
    soap DeletePortMapping "<NewRemoteHost></NewRemoteHost><NewExternalPort>$PUERTO</NewExternalPort><NewProtocol>TCP</NewProtocol>" >/dev/null
    echo "Puerto $PUERTO cerrado."
    exit
fi
MOVIL="${1:?indica la IP del móvil, p. ej. 192.168.20.3}"
PUERTO="${2:-7900}"
soap AddPortMapping "<NewRemoteHost></NewRemoteHost><NewExternalPort>$PUERTO</NewExternalPort><NewProtocol>TCP</NewProtocol><NewInternalPort>$PUERTO</NewInternalPort><NewInternalClient>$MOVIL</NewInternalClient><NewEnabled>1</NewEnabled><NewPortMappingDescription>rele mobilemesh</NewPortMappingDescription><NewLeaseDuration>0</NewLeaseDuration>" >/dev/null
soap GetSpecificPortMappingEntry "<NewRemoteHost></NewRemoteHost><NewExternalPort>$PUERTO</NewExternalPort><NewProtocol>TCP</NewProtocol>" | grep -q "<NewInternalClient>$MOVIL<" \
    && echo "Puerto $PUERTO abierto hacia $MOVIL. Dirección pública: $(soap GetExternalIPAddress '' | sed -n 's/.*<NewExternalIPAddress>\([^<]*\).*/\1/p')" \
    || { echo "El router no aceptó abrir el puerto."; exit 1; }
