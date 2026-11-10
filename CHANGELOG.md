# Changelog

Usamos [semver](https://semver.org/lang/es/). En 0.x, el número del medio puede romper cosas y el último solo
arregla. Para no llevarte sorpresas fija la versión exacta: `"soblendwa-node": "0.2.0"` (sin `^`),
`soblendwa = "=0.2.0"`, `soblendwa==0.2.0`.

## 0.2.0 (sin publicar)

### Rompe

- `Event::LoggedOut` trae `cause` y `code`. En Rust: `Event::LoggedOut { reason, .. }`.
- `profile_picture_url` falla con `PRIVATE_PHOTO` si la foto es privada; `None` ya solo es "sin foto".
- `join_group` falla con `APPROVAL_REQUIRED` cuando el grupo pide aprobación (la solicitud igual se manda).
- Node, Python y servidor: parámetros malos → `INVALID_ARGUMENT`, mensaje inexistente → `MESSAGE_NOT_FOUND` (antes
  ambos `OTHER`).

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
- Rust: `send_prepared(chat, future)` es público.
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
- Dos hilos podían guardar la misma sesión Signal en orden invertido y dejar un ratchet viejo en el store. La ventana
  era de nanosegundos y nunca se vio, pero ahora cada sesión se guarda antes de soltar su lock.
- Node: un listener que tiraba excepción cortaba los eventos, y una promesa rechazada podía tumbar el proceso.
- Python: una excepción en un handler paraba `run()`.

## 0.1.0 (2026-11-05)

Primera versión en crates.io, npm (`soblendwa-node`, `soblendwa-client`) y PyPI.
