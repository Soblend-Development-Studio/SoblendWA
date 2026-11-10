# Changelog

Seguimos [semver](https://semver.org/lang/es/). Mientras estemos en 0.x, subir el número del medio (0.1 → 0.2) puede
romper algo de tu código y subir el último (0.2.0 → 0.2.1) solo arregla bugs. Si WhatsApp cambia algo y se rompe la
conexión, el arreglo sale como parche.

Si no quieres sorpresas, fija la versión exacta: `"soblendwa-node": "0.2.0"` sin `^` en npm, `soblendwa = "=0.2.0"`
en Cargo, `soblendwa==0.2.0` en pip.

## 0.2.0 (sin publicar)

Cambios que pueden romper tu código:

- `Event::LoggedOut` ahora trae `cause` y `code`. En Rust usa `Event::LoggedOut { reason, .. }`.
- `profile_picture_url` da error `PRIVATE_PHOTO` si la foto es privada. `None` / `null` ya solo quiere decir que no hay
  foto.
- `join_group` da error `APPROVAL_REQUIRED` si el grupo pide aprobación (la solicitud sí se manda).
- En Node, Python y el servidor, los parámetros malos dan `INVALID_ARGUMENT` y un mensaje que no existe da
  `MESSAGE_NOT_FOUND`. Antes los dos eran `OTHER`.

Nuevo:

- Errores con código estable (`error.code`) y estado HTTP de WhatsApp cuando lo hay (`error.status`). En Rust,
  `Error::code()` y `Error::status()`. En Node, Python y el cliente JS llegan como `SoblendError`.
- Evento `joinedGroup`, sin importar si al bot lo metieron al crear el grupo, lo agregaron o entró por enlace. También
  `leftGroup`.
- Evento `lidMapping` cuando se descubre que un número y un LID son la misma persona.
- `messageLost` cuando un mensaje que no se pudo descifrar no se recupera en `retry_deadline` (10 min). Se cuentan en
  `stats().lost`, que baja si alguno llega tarde.
- `loggedOut` dice la causa: `removedFromPhone`, `sessionInvalidated` o `userLogout`.
- `stats().sendTimings`: p50, p95 y máximo de cada fase de los últimos 512 envíos. Se recalcula solo cuando llegan
  tiempos nuevos, así que `stats()` sigue costando ~2 µs en Rust.
- `mediaStore` / `media_store`: varias cuentas comparten la multimedia ya subida, también después de reiniciar.
- Node: todas las acciones de grupo, perfil, privacidad, `lidForPhone` / `phoneForLid`, menciones en `sendText` y en
  multimedia, miniatura propia como `Buffer`, `restart()`, `close()`, `waitFor()` y `call()`.
- Node: `FakeWhatsApp` para tests, con grupos, invitaciones, aprobación y fallos provocados.
- Node: `importZapo` y `rollbackZapo` para SQLite y Mongo, con `check: true` para probar sin escribir.
- Python, servidor y cliente JS con los mismos métodos.
- El WhatsApp falso avisa de grupos creados, altas, bajas, salidas, cambios de nombre e invitaciones.
- `sendLimit` en Node (`send_limit` en Python): `false` lo apaga, o `{ perMinute, burst, perChatPerMinute, perChatBurst }`
  para ajustarlo. Antes solo se podía cambiar desde Rust.
- Error `NO_DEVICES` (`Error::NoDevices`) cuando el destinatario no tiene ningún dispositivo con WhatsApp. Antes era
  `OTHER`.
- `store-check.js` y `live-check.js` en el paquete de Node, para probar tu base de datos y un número de prueba. Ver
  [Probar en real](https://soblend-development-studio.github.io/SoblendWA/#probar-en-real).
- Pruebas contra PostgreSQL y Redis reales (`SOBLEND_POSTGRES_URL`, `SOBLEND_REDIS_URL`), y un trabajo de CI que corre
  las de MongoDB, PostgreSQL y Redis contra servidores de verdad en cada push.
- Logo nuevo: una S roja sobre una burbuja negra. Es el mismo en el README, los banners, la documentación y los
  paquetes, y hay una imagen para cuando compartes el enlace (`assets/social-preview.png`). La documentación pasa a
  rojo, y su tema oscuro a negro.
- `examples/soak.rs` para dejar un número conectado semanas: no aparece en línea, no responde a nadie y solo contesta
  `!yowner` del dueño en un chat privado con el resumen de la prueba. El reporte no guarda textos ni números.

Más rápido (medido en la misma máquina, ver `pipeline` y `throughput` en las pruebas ignoradas):

- Node y Python reciben los eventos en lotes, en un solo texto JSON por lote: los mensajes llegan al handler el doble de
  rápido y con ~35% menos CPU.
- El JSON de mensajes, recibos, confirmaciones y presencia se escribe directo, sin armar un árbol intermedio: de ~18,7
  µs a ~3 µs por mensaje.
- Cargar una sesión Signal guardada ya no recalcula su clave pública con una multiplicación de curva: de ~54 µs a ~4 µs.
  Las sesiones guardadas antes se leen igual.
- Firmar mensajes de grupo reutiliza la clave de Edwards de cada sender key: cifrar y descifrar en grupo, ~18% más.
- Los módulos de Node y Python y el servidor usan mimalloc. En Windows y en Linux con musl el asignador del sistema era
  lo que más pesaba en el camino de un mensaje.
- Formatear un JID ya no pasa por `write!`.
- La miniatura de una foto se hace con un muestreo pensado para reducciones grandes: de ~93 ms a ~41 ms en una foto
  de 1600×1200.

Arreglado:

- El aviso `create` de un grupo nuevo llegaba sin participantes, así que un saludo que esperaba `add` nunca salía.
- Vincular con código (`request_pair_code`, `requestPairCode`) fallaba con `400 bad-request`: WhatsApp exige que el
  nombre del dispositivo sea `Navegador (Sistema)` y se mandaba `Chrome (SoblendWA)`. Ahora va `Chrome (Windows)`,
  `Chrome (Mac OS)` o `Chrome (Linux)` según dónde corra. Probado con un teléfono real.
- Los futures de `send_text`, `send_message` y `send_image` ocupaban 25 a 31 KB cada uno, en debug y en release.
  Mandar una imagen desde el hilo principal en Windows (1 MB de pila) terminaba en un desbordamiento de pila. Ahora el
  trabajo pesado va en el heap y esos futures ocupan menos de 2 KB; hay una prueba que lo vigila.
- `connect()` justo después de `disconnect()` dejaba la cuenta desconectada.
- Si una persona aparecía sin dispositivos (por ejemplo, al precargar un grupo justo cuando se registraba), esa lista
  vacía se quedaba en caché y cada envío a ella fallaba durante 15 minutos. Ahora se vuelve a preguntar a los 5
  segundos, y si llega un mensaje de un dispositivo que la caché no conoce, la lista se renueva en ese momento. Eso
  también cubre cuando alguien abre WhatsApp Web y te escribe desde ahí antes de que llegue el aviso de dispositivos.
- Varios envíos al mismo chat sin esperar uno por uno (`bot.sendText(...)` sin `await`, `asyncio.gather` o llamadas
  por WebSocket) podían salir en otro orden: en una ráfaga de 300, unos 30 llegaban cambiados de lugar. Ahora cada
  envío toma su turno en el momento en que lo llamas y sale en ese orden; los chats distintos siguen en paralelo.
- Node: un listener que lanzaba excepción cortaba los eventos de esa cuenta, y una promesa rechazada sin listener de
  `error` podía tumbar el proceso con todas las cuentas.
- Python: una excepción en un handler paraba `run()`.

## 0.1.0 (2026-11-05)

Primera versión en crates.io, npm (`soblendwa-node`, `soblendwa-client`) y PyPI.
