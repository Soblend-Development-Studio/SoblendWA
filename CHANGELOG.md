# Changelog

Usamos [semver](https://semver.org/lang/es/). En 0.x, el número del medio puede romper cosas y el último solo
arregla. Para no llevarte sorpresas fija la versión exacta que instalas, sin `^`: `"soblendwa-node": "X.Y.Z"`,
`soblendwa = "=X.Y.Z"`, `soblendwa==X.Y.Z`. Una versión dice "sin publicar" hasta que sale; entonces lleva su fecha.

## 0.2.0 (sin publicar)

### Rompe

- `Event::LoggedOut` trae `cause` y `code`. En Rust: `Event::LoggedOut { reason, .. }`.
- `profile_picture_url` falla con `PRIVATE_PHOTO` si la foto es privada; `None` ya solo es "sin foto".
- `join_group` falla con `APPROVAL_REQUIRED` cuando el grupo pide aprobación (la solicitud igual se manda).
- Node, Python y servidor: parámetros malos → `INVALID_ARGUMENT`, mensaje inexistente → `MESSAGE_NOT_FOUND` (antes
  ambos `OTHER`).
- Un envío que pasa el límite ya no falla en el acto: espera en la fila de su chat hasta 30 s (`max_wait`) y solo
  entonces da `RATE_LIMITED`. `max_wait: 0` / `maxWaitSecs: 0` deja el comportamiento de antes.
- `SendLimit` tiene campos nuevos (`per_group_per_minute`, `per_group_burst`, `max_wait`); si lo armas a mano, termina
  con `..Default::default()`.
- Node, Python y servidor: una opción desconocida da `INVALID_ARGUMENT` con la más parecida (`did you mean
  "sendLimit"?`) en lugar de ignorarse.

### Nuevo

- Errores con código estable y status de WhatsApp: `Error::code()` / `Error::status()`, `SoblendError` en Node,
  Python y el cliente JS. Nuevo `NO_DEVICES` para números sin WhatsApp.
- Eventos `joinedGroup` / `leftGroup` (da igual si fue `create`, `add` o enlace), `lidMapping` y `messageLost`.
- `loggedOut.cause`: `removedFromPhone`, `sessionInvalidated` o `userLogout`.
- `stats().lost` y `stats().sendTimings` (p50/p95/máx por fase, últimos 512 envíos).
- `mediaStore`: multimedia subida compartida entre cuentas y entre reinicios.
- `sendLimit` desde Node y Python (`false` para apagarlo).
- Node: acciones de grupo, perfil, privacidad, `lidForPhone` / `phoneForLid`, menciones, miniatura propia como `Buffer`,
  `restart()`, `close()`, `waitFor()`, `call()`, `FakeWhatsApp`, `importZapo` / `rollbackZapo`.
- Python, servidor y cliente JS al mismo nivel que Node.
- Python: `FakeWhatsApp` y `wait_for()`, igual que en Node. `test_fake.py` corre en el CI junto con `test-fake.js`.
- Servidor: `GET /metrics` en formato Prometheus, con la misma clave que lo demás.
- `FakeWhatsApp::slow_uploads` / `slowUploads(ms)` para simular un CDN lento.
- Rust: `send_prepared(chat, future)` es público, y `connect_timeout` / `keepalive` en el builder.
- Todas las opciones de Rust en Node, Python y el servidor: `outbox`, `mediaCache`, `workers`, `queryTimeoutSecs`,
  `connectTimeoutSecs`, `keepaliveSecs` y `version`. El servidor las recibe en `--options` (JSON o `@archivo.json`).
  Una prueba compara `Config` con lo que aceptan los bindings.
- Límite propio para grupos (`per_group_per_minute`, `per_group_burst`) y `stats().rate_delayed` /
  `soblendwa_rate_delayed_total` para los envíos que esperaron su turno.
- `tests/zapo/round-trip.cjs`: zapo-js 1.11 escribe una sesión con su propio `@zapo-js/store-mongo` (o `store-sqlite`),
  SoblendWA la importa, sigue las mismas conversaciones sin sesiones nuevas y la devuelve a zapo, que continúa. Corre en
  el CI contra MongoDB 7.
- `tests/zapo/versus.cjs`: la comparación con zapo con SQLite en los dos, una red con latencia y jitter de por medio,
  documentos subidos en cada pedido, un grupo y el arranque desde la base.
- `bindings/node/soak.js`: la prueba larga desde Node.js, con la misma carpeta y el mismo reporte que `examples/soak.rs`.
- Un `stream:error` trae en el motivo de la desconexión el nodo completo y, si quedó algo sin confirmar, cuál era.
- `tests/zapo/dry-run.cjs`: el simulacro de migración sobre todas las sesiones de una base de zapo, sin conectar nada.
  Con la base de un bot real: 27 sesiones, 0 errores, 1,5 s.
- `versus.cjs --via usuario@host`: la misma comparación con la red de verdad de por medio, por un túnel SSH de ida y
  vuelta. La red se mide con eco sobre una conexión abierta (la primera versión cronometraba una conexión nueva y daba
  650 ms, más de lo que tardaba el bot en contestar), y el reporte guarda la fecha y el rango de cada número.
- Con la feature `testing`: `simulate_outage()` y `damage_sessions(jid)` para probar con un número real el outbox y el
  reintento; en `soak.js`, `!yowner cola`, `!yowner mac`, `!yowner media` y `!yowner reiniciar`.
- Guía de equivalencias desde zapo, y cómo se detecta cada causa de `loggedOut`.
- `soak.js` repite las pruebas en vivo y las cuenta con un testigo, un segundo dispositivo del mismo número con
  `examples/soak.rs --comando`: `!yowner caida N` (5 mensajes y `kill -9` a mitad del envío), `!yowner compartida N`
  (multimedia compartida entre dispositivos tras un `kill -9`), `!yowner mac` repetible, `!yowner pruebas` con la tabla
  y `--por-dia` con una fila por día y por proceso. Los comandos que WhatsApp vuelve a entregar tras un `kill -9` no se
  repiten, y cada conexión pide su propio código para vincular. Con el testigo en otro número (`--numero-testigo` y
  `--testigo-de`) la multimedia compartida se prueba entre dos cuentas y `!yowner mac 10` corre sin teléfono. En un
  servidor, `<carpeta>/orden` y `SIGUSR2` lanzan cualquier comando sin escribir en WhatsApp. Con dos cuentas reales:
  `kill -9` 10 de 10 sin perder ni duplicar, MAC dañada 10 de 10 recuperada, multimedia compartida 5 de 5.
- `published.yml`: después de cada publicación instala desde npm, PyPI y crates.io en máquinas limpias y conecta hasta
  recibir el QR.
- La documentación dice "0.2.0 (sin publicar)" en el encabezado, el pie y las tablas hasta que la versión tiene fecha.
- La fuente de la documentación vive en `site/`: la versión sale de `Cargo.toml`, el historial de este archivo, y el
  CI falla si `docs/index.html` quedó viejo, si hay un enlace roto, si una página menciona una versión que no es la
  actual o si un ejemplo de Rust no compila.
- `store-check.js` y `live-check.js` para probar tu base y un número de prueba;
  [guía](https://soblend-development-studio.github.io/SoblendWA/#probar-en-real).
- `examples/soak.rs`: deja tu número conectado semanas y te contesta solo a ti con `!yowner`.
- Pruebas contra PostgreSQL y Redis reales; el CI corre MongoDB 7, PostgreSQL 16 y Redis 7 en cada push.
- Logo y banners nuevos, docs en rojo con tema oscuro negro, `assets/social-preview.png`, `SECURITY.md`.

### Más rápido

Medido en la misma máquina antes y después (`pipeline` y `throughput` en las pruebas ignoradas).

- Eventos hacia Node y Python en lotes, un JSON por lote: el doble de mensajes por segundo al handler, ~35% menos CPU.
- JSON de mensajes, recibos y acks escrito directo: 18,7 µs → 3 µs.
- Cargar una sesión Signal sin rehacer su clave pública: 54 µs → 4 µs. Las sesiones viejas cargan igual.
- Firma de grupo con la clave de Edwards cacheada: ~18% más en grupos.
- mimalloc en Node, Python y el servidor.
- Miniaturas: 93 ms → 41 ms para una foto de 1600×1200.
- Varios archivos al mismo chat suben a la vez y se entregan en orden. Seis subidas de 150 ms: 1,08 s → 0,36 s
  (`uploads_overlap_in_order`). Vale para Rust, Node, Python y el servidor.
- Signal con un lock por contacto en vez de uno por cuenta (`signal::bench::contention`, release): 32 chats recién
  reiniciados con un store de 5 ms por lectura, 490 ms → 7 ms; con todo en caché, 99 mil → 210 mil descifrados/s en
  un hilo y 78 mil → 583 mil en cuatro. Los envíos a grupos también leen sus sesiones a la vez.
- JIDs formateados sin `write!`.

### Arreglado

- `create` de un grupo nuevo llegaba sin participantes (el saludo que esperaba `add` nunca salía).
- Vincular con código daba `400 bad-request`: hay que mandar `Navegador (Sistema)`, mandábamos `Chrome (SoblendWA)`.
  Probado con un teléfono real.
- `send_text` / `send_image` ocupaban 25–31 KB de future y podían desbordar la pila del hilo principal en Windows.
  Ahora < 2 KB, con una prueba que lo vigila.
- Una lista de dispositivos vacía quedaba en caché ~15 min y todo envío a esa persona fallaba. Ahora se vuelve a
  preguntar a los 5 s, y un mensaje de un dispositivo desconocido renueva la lista.
- Envíos al mismo chat sin `await` (o con `asyncio.gather`, o por WebSocket) podían salir desordenados: ~30 de 300.
  Ahora salen en el orden en que se llamaron.
- `connect()` justo después de `disconnect()` se quedaba desconectado.
- Un mensaje que esperaba su reintento se olvidaba al reiniciar: no llegaba ni `messageLost` ni `messageRecovered`, y
  `stats().lost` se quedaba corto. Ahora la espera se guarda en el store (`awaiting-retry`).
- Python: si salías de `events()` a la mitad de un lote, el resto del lote se perdía.
- Con la sesión dañada (`bad message mac`), un mensaje necesitaba dos reintentos: el primero no llevaba llaves y el
  teléfono reenviaba por la misma sesión. Ahora las lleva desde el primero cuando falló un `msg` o `pkmsg`, y
  `undecryptableMessage` sale una sola vez por mensaje aunque el reenvío también falle. Visto con `!yowner mac`; con
  el arreglo, la misma prueba se recupera con un solo reintento, en 0,8 s.
- `!yowner reiniciar` en `soak.js` salía sin guardar los contadores, y el resumen perdía lo que pasó desde la última
  foto.
- `send_*` devuelve el id antes de que WhatsApp confirme, pero el outbox se escribía hasta 50 ms después (los stores
  escriben en lote). Un `kill -9` en ese hueco perdía un mensaje que ya se había dado por enviado, junto con el avance
  de la sesión Signal. Ahora la entrada del outbox se escribe antes de mandar.
- Las reacciones a tus estados llegan como `<status type="reaction">` y no se confirmaban: WhatsApp cortaba la conexión
  50 minutos después y la volvía a mandar en cada reconexión, para siempre. Lo encontró la prueba larga con un número
  real (17 reconexiones forzadas en 19 horas) una vez que el motivo de la desconexión empezó a traer el nodo completo.
  Con el arreglo, la misma cuenta pasó los 50 minutos sin cortes.
- Después de un corte, el outbox reenviaba en desorden los mensajes del mismo segundo (`created_at` estaba en segundos).
  Ahora cada mensaje lleva un orden en microsegundos que nunca se repite.
- El primer archivo después de conectar esperaba un viaje más por los hosts de subida (`media_conn`); ahora se piden al
  conectar. Y una subida que falla por una conexión ya cerrada reintenta sin volver a pedir hosts, salvo que WhatsApp
  rechace la autorización. El p95 de documentos contra zapo pasó de 422 contra 285 ms a 213 contra 250 ms.
- Dos hilos podían guardar la misma sesión Signal en orden invertido y dejar un ratchet viejo en el store. La ventana
  era de nanosegundos y nunca se vio, pero ahora cada sesión se guarda antes de soltar su lock.
- Node: un listener que tiraba excepción cortaba los eventos, y una promesa rechazada podía tumbar el proceso.
- Python: una excepción en un handler paraba `run()`.

## 0.1.0 (2026-11-05)

Primera versión en crates.io, npm (`soblendwa-node`, `soblendwa-client`) y PyPI.
