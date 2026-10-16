#!/bin/sh
# Instala el relé de MobileMesh en un servidor Linux (Oracle Cloud, cualquier VPS o un PC de casa) y
# lo deja arrancando solo, también tras reiniciar. Uso, en el servidor y como administrador:
#
#   sudo sh instalar-rele.sh ./mesh-rele [puerto]
#
# El relé no guarda nada más que buzones cifrados (máximo 7 días) y no escribe registros.
set -eu
BIN="${1:?indica la ruta del programa mesh-rele}"
PUERTO="${2:-7900}"

install -m 755 "$BIN" /usr/local/bin/mesh-rele
id mesh-rele >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin mesh-rele

cat > /etc/systemd/system/mesh-rele.service <<EOF
[Unit]
Description=Relé de MobileMesh
After=network-online.target
Wants=network-online.target

[Service]
User=mesh-rele
ExecStart=/usr/local/bin/mesh-rele 0.0.0.0:$PUERTO
Restart=always
RestartSec=3
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
EOF

# Cortafuegos del propio servidor (las imágenes de Oracle traen iptables cerrado).
if command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
    firewall-cmd --permanent --add-port="$PUERTO/tcp" && firewall-cmd --reload
elif command -v iptables >/dev/null 2>&1; then
    iptables -C INPUT -p tcp --dport "$PUERTO" -j ACCEPT 2>/dev/null || iptables -I INPUT 1 -p tcp --dport "$PUERTO" -j ACCEPT
    if command -v netfilter-persistent >/dev/null 2>&1; then netfilter-persistent save; fi
fi

systemctl daemon-reload
systemctl enable --now mesh-rele
sleep 1
systemctl --no-pager --lines=3 status mesh-rele
echo
echo "Relé instalado en el puerto $PUERTO. Recuerda abrir también ese puerto (TCP) en el panel del"
echo "proveedor (en Oracle: Red virtual → Lista de seguridad → Regla de entrada)."
