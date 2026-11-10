# Seguridad

SoblendWA guarda claves de cifrado y sesiones de WhatsApp, así que tomamos en serio cualquier falla.

## Cómo reportar algo

No abras un issue público. Usa el botón **Report a vulnerability** en la pestaña
[Security](https://github.com/Soblend-Development-Studio/SoblendWA/security) del repositorio, que nos llega en
privado. Cuéntanos qué encontraste, cómo reproducirlo y qué versión usas. Te contestamos en cuanto lo veamos y, si se
confirma, sale un parche con su entrada en el [CHANGELOG](CHANGELOG.md).

## Versiones

Mientras estemos en 0.x solo se arregla la última versión publicada.

## Qué está revisado y qué no

- El cifrado (Signal, sender keys y el handshake Noise) **no tiene auditoría externa**. Lo que sí hay: interoperabilidad
  probada contra libsignal-node y contra zapo-js en los dos sentidos, y verificación de la cadena de certificados de
  WhatsApp en cada conexión.
- El decodificador binario se prueba con cientos de miles de entradas aleatorias y mutadas; tiene límites de
  profundidad y de descompresión para que un mensaje hostil no tumbe el proceso.
- No hay código `unsafe` en la librería (`#![forbid(unsafe_code)]`).
- Las vistas previas de enlaces nunca consultan hosts privados (`localhost`, la red interna, `169.254.169.254`),
  tampoco por redirección ni por DNS.

## Para tu bot

- La carpeta o la base donde guardas la sesión tiene tus claves privadas: quien la copie puede leer y mandar mensajes
  como tú. Dale permisos solo a tu usuario y no la subas a git.
- `soblendwa-server` escucha en `127.0.0.1` por defecto. Si lo expones, usa siempre `--token` y ponlo detrás de TLS.
- Automatizar una cuenta puede terminar en bloqueo de WhatsApp; eso no es una falla de seguridad, pero tenlo en cuenta.
