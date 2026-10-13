# Fase 0: reglas, límites y amenazas

## Principios (obligatorios)

1. Ningún servidor central obligatorio guarda el contenido privado de los usuarios.
2. Los datos se guardan en el dispositivo por defecto.
3. Entre dispositivos todo viaja cifrado de extremo a extremo.
4. Las claves privadas no salen nunca del dispositivo; el usuario controla sus copias de recuperación.
5. La conexión directa (P2P) es el camino preferido.
6. Cualquier servicio auxiliar (descubrimiento, señalización, relé, avisos) se puede cambiar o alojar uno mismo.
7. Las funciones locales siguen funcionando sin conexión.
8. Un mensaje solo se marca «entregado» cuando el destino confirma que lo **guardó**.

## Lo que MobileMesh NO promete

- Si se pierden todas las copias de un dato, el dato se pierde.
- Si el destinatario está apagado y nadie guarda el mensaje por él, no hay entrega inmediata.
- Un relé que reenvía tráfico cifrado **sí** ve metadatos: direcciones IP, horas y quién habla con quién.
- No hay «olvidé mi contraseña»: sin dispositivo, copia de recuperación ni dispositivos vinculados, la cuenta no se recupera.
- Borrar algo en el teléfono no borra al instante las copias que tengan otros dispositivos desconectados.

## Modelo de amenazas

| Atacante | Qué puede hacer | Defensa | Estado |
|---|---|---|---|
| Espía en la red o relé | Leer y guardar el tráfico | Cifrado Noise (ChaCha20-Poly1305), claves nuevas en cada sesión | Probado (`intermediario_solo_ve_datos_cifrados`) |
| Impostor en la dirección del contacto | Hacerse pasar por él | Se exige su clave pública antes de revelar la nuestra | Probado (`suplantacion_detectada…`) |
| Desconocido que conoce mi clave | Enviarme mensajes o spam | Solo se aceptan contactos guardados | Probado (`desconocido_es_rechazado`) |
| Alguien que altera o repite paquetes | Manipular o duplicar mensajes | Noise en el canal y Olm en cada mensaje; un mensaje falso no gasta claves | Probado (`mensaje_alterado_repetido…`) |
| Reintentos legítimos | Mensajes duplicados | Cada mensaje tiene identificador único; el receptor no duplica | Probado (`sin_conexion_queda_pendiente…`) |
| Ladrón del teléfono | Leer la base de datos y la clave | Android: identidad cifrada con Keystore (solo con el teléfono desbloqueado), carpeta privada, sin copia automática a Google. PC: archivo 0600 | Hecho y verificado en teléfono; falta cifrar la base de datos |
| Respondedor curioso | Ver la clave pública de quien atiende | En Noise XX quien escucha muestra su clave pública a quien conecta | Aceptado: es pública |
| Apagón o batería agotada al crear la identidad | Dejar la clave a medias (pasó: 64 bytes a cero en el TECNO) | Se graba aparte, se fuerza al disco y se renombra; al abrir se comprueba que la clave pública corresponde a la privada | Probado (`identidad_danada_se_detecta`) y verificado en teléfono |
| Alguien fotografía la copia de recuperación | Robar la identidad | La copia va cifrada con contraseña (Argon2id + ChaCha20-Poly1305) | Probado (`copia_y_restauracion`) y verificado en teléfono |
| Alguien cambia la clave de un contacto guardado | Suplantarlo en silencio | El cambio se rechaza; hay que comprobarlo y añadirlo aparte | Probado (`base_antigua_se_migra…`) |
| Alguien dice ser un dispositivo de mi contacto | Suplantarlo con su propia clave | Cada dispositivo presenta un certificado firmado por la cuenta; se comprueba en el saludo | Probado (`certificado_falso_o_ajeno_se_rechaza`) |
| Roban un dispositivo vinculado | Seguir usando la cuenta | Solo el principal tiene la clave de la cuenta; el robado se **retira** con una lista firmada que viaja en cada conexión | Probado (`dispositivo_retirado_es_rechazado_por_todos`) y en el TECNO |
| Un contacto molesto o una inundación | Spam, saturar el teléfono | Bloqueo, 60 mensajes/min por contacto, 30 conexiones/min por IP | Probado (`contacto_bloqueado…`, `demasiados_mensajes…`) y bloqueo en el TECNO |
| Un contacto con quien comparto datos | Cambiar lo que solo puede leer o firmar a nombre de otro | Permiso por colección y autoría comprobada en cada cambio | Probado (`lista_compartida_respeta_los_permisos`) y en el TECNO |
| Alguien altera o corta un archivo | Que se acepte un archivo dañado | Cada trozo autenticado con la clave del archivo y huella SHA-256 al final | Probado (`archivo_alterado_se_rechaza`) y huella verificada en teléfono |
| Copia del teléfono | Ver las fotos recibidas | Se guardan cifradas; solo se descifran en memoria o al pedir «Guardar en Descargas» | Verificado en teléfono |
| Quien guarda mi copia | Leerla, cambiarla o colar una vieja | Cifrada con la clave de la cuenta; cuenta y versión autenticadas; comprobación por huella con número al azar | Probado (`copia_alterada_o_vieja_se_detecta`) y verificado con el PC |
| Un tercero pide mi copia | Llevársela | Solo se entrega a quien demuestra la cuenta en el saludo | Probado (`sin_permiso_no_se_guarda_y_un_ajeno_no_descarga`) |
| Relé | Leer o suplantar | Noise de extremo a extremo a través del relé | Probado (`el_rele_une_dispositivos…`) y en teléfonos |
| Relé u observador de red | Metadatos: IP, horas, volumen, quién llama a qué buzón | Buzones al azar sin relación con la cuenta, sin registros, relleno a 512 bytes | Parcial: buzones rotativos y control cifrado pendientes |
| Servidor de avisos | Ver metadatos | Avisos sin contenido | Pendiente (Fase 11) |
| Mensajes guardados para alguien desconectado | Leerlos | Cifrado por mensaje con Double Ratchet (Olm, vodozemac) dentro del canal Noise | Hecho; guardarlos en relés llega en la Fase 9 |
| Alguien copia la base de datos | Leer los mensajes | Texto y estado del ratchet cifrados con claves derivadas de la del dispositivo | Probado (`la_base_de_datos_copiada_no_revela_el_texto`) y verificado en teléfono |
| Cambia la clave de cifrado de un dispositivo | Interceptar mensajes | Clave fijada la primera vez; si cambia, se rechaza | Probado (`cambio_de_clave_de_cifrado…`) |

## Criterios verificables de privacidad

- Un intermediario que reenvía todo el tráfico no encuentra ni el texto ni la estructura del mensaje (prueba automática).
- Conectar con alguien cuya clave no coincide falla **antes** de enviar nada nuestro.
- Ningún comando ni prueba usa servidores externos.
