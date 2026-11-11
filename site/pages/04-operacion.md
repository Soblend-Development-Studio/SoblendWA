=== confiabilidad | Confiabilidad | Operación
lead: Pensado para bots que corren meses sin que nadie los mire, con varias cuentas en el mismo proceso.

## Si WhatsApp bloquea la cuenta {#confiabilidad-bloqueo}

Se detiene y te dice por qué, sin borrar la sesión. Si desbloqueas la cuenta en el teléfono, vuelves a conectar sin escanear otro QR.

| Código | Significado |
|---|---|
| `402` | Baneo temporal |
| `403` | Cuenta bloqueada |
| `406` | Baneo |

```rust
if let Event::AccountBlocked { code, reason, expires } = event {
    eprintln!("Bloqueada ({code}): {reason}. Hasta: {expires:?}");
}
```

## Reconexión {#confiabilidad-reconexion}

La espera crece y tiene un margen aleatorio, para que cien sesiones no se reconecten en el mismo milisegundo.

| Fallo seguido | Espera |
|---|---|
| Primero | De 0 a 0,75 s |
| Segundo | 1 s más un margen aleatorio |
| Tercero | 2 s más un margen aleatorio |
| Siguientes | 4 s, 8 s… hasta 60 s, más un margen aleatorio |

Se apaga con `.auto_reconnect(false)`. El keepalive envía un ping cada 15 segundos y, si no hay respuesta en 8, reconecta. El servidor además escalona el arranque de las sesiones.

## Límite de envíos {#confiabilidad-limite}

Un bug que manda mensajes en bucle no se convierte en spam. Cada envío gasta una ficha del total de la cuenta y otra de su chat; las fichas vuelven poco a poco, al ritmo por minuto de cada límite.

| Campo de `SendLimit` | Por defecto | Qué limita |
|---|---|---|
| `per_minute`, `burst` | 240, 60 | Mensajes por minuto y ráfaga máxima, en total |
| `per_chat_per_minute`, `per_chat_burst` | 40, 15 | Lo mismo en cada chat privado |
| `per_group_per_minute`, `per_group_burst` | 40, 15 | Lo mismo en cada grupo |
| `max_wait` | 30 s | Cuánto puede esperar un envío que pasó el límite |

### Qué pasa al pasarse

El envío **espera en la fila de su chat** hasta que le toque una ficha, y sale en el mismo orden en que lo pediste. Los demás chats siguen sin esperar, salvo que se acabe el límite total. Así una ráfaga de comandos en un grupo activo se reparte en el tiempo en lugar de fallar:

- Con el límite por defecto, un grupo puede recibir 15 mensajes seguidos y después uno cada 1,5 s.
- Cada envío que tuvo que esperar se cuenta en `stats().rate_delayed`, y su espera aparece en la fase `queued` de los [tiempos por fase](#confiabilidad-tiempos-por-fase).
- Si la ficha no llega dentro de `max_wait`, contando desde que llamaste, falla con `Error::RateLimited { chat, retry_after }` (código `RATE_LIMITED`) y se cuenta en `stats().rate_limited`. Si desde el principio se ve que no va a alcanzar, falla en el acto en lugar de esperar para nada.
- Con `max_wait` en cero vuelve el comportamiento de antes: falla en el acto, antes de subir ningún archivo.

Si tu bot vive en grupos muy activos, sube solo el límite de grupos y deja igual el de chats privados, que es donde un bucle hace más daño:

```rust
use std::time::Duration;
use soblendwa::SendLimit;

let limite = SendLimit {
    per_group_per_minute: 90,
    per_group_burst: 30,
    max_wait: Duration::from_secs(20),
    ..Default::default()
};
let (client, events) = Client::builder().store(store).send_limit(Some(limite)).build().await?;
```

`.send_limit(None)` lo desactiva. Bajar los límites reduce el riesgo, pero no hay ningún ritmo que garantice que WhatsApp no bloquee una cuenta, y nadie publica cuánto tolera en un grupo.

En Node.js es la opción `sendLimit` y en Python `send_limit`: `false` lo apaga, y un objeto cambia solo los campos que pongas. `max_wait` se escribe en segundos: `maxWaitSecs`.

:::tabs
```js Node.js
const bot = await SoblendWA.create({ sendLimit: { perGroupPerMinute: 90, perGroupBurst: 30, maxWaitSecs: 20 } })
```
```python Python
bot = await SoblendWA.create(send_limit={"per_group_per_minute": 90, "per_group_burst": 30, "max_wait_secs": 20})
```
```bash Servidor
soblendwa-server --options '{"sendLimit": {"perGroupPerMinute": 90, "maxWaitSecs": 20}}'
```
:::

## Reinicios sin pérdidas {#confiabilidad-outbox}

Cada mensaje se guarda en un outbox hasta que WhatsApp confirma que lo recibió. Si se cae la conexión o reinicias el proceso, se reenvía al reconectar **con el mismo id**, y un mensaje que sigue en vuelo no se reenvía.

- `client.pending_sends()` muestra lo que está esperando: `id`, `chat`, `queued_at` y `attempts`.
- `stats().outbox_resent` cuenta los reenvíos.
- `.outbox(false)` lo desactiva.

:::note Si tu bot usa otra librería al otro lado
Un reenvío lleva el mismo id. SoblendWA descarta los duplicados al recibir, y los bots hechos con otras librerías deben hacer lo mismo.
:::

## Reintentos después de reiniciar

Lo enviado se conserva hasta dos días, así un contacto que no pudo descifrar un mensaje puede pedirlo de nuevo aunque el bot se haya reiniciado.

Al revés también: los mensajes que el bot no pudo descifrar y siguen esperando su reintento se guardan en el store (`awaiting-retry`). Si reinicias a la mitad, la espera sigue donde estaba, y `MessageLost` o `MessageRecovered` llegan igual.

## Un fallo no tumba todo

Si un handler entra en pánico, se registra, se cuenta en `stats().panics` y la sesión sigue. Cada sesión del servidor es independiente: si una se cuelga abriendo su almacenamiento, las demás siguen.

## Recuperación de mensajes

Si un mensaje no se puede descifrar, SoblendWA pide un reintento al remitente y una copia a tu teléfono a la vez. Cuando llega bien, emite `Event::MessageRecovered`. Si no llega dentro del plazo, emite `Event::MessageLost` y lo cuenta en `stats().lost`, así sabes cuántos se perdieron de verdad. Los detalles están en [Recibir mensajes](#recibir-mensajes-que-no-se-pudieron-descifrar). Esto está probado de punta a punta contra el WhatsApp falso, y aún falta confirmarlo con una cuenta real. {exp}

## Varias cuentas en un proceso {#confiabilidad-varias-cuentas}

Cada cuenta es un cliente independiente. Si una se desconecta, la bloquean o un handler suyo falla, las demás siguen.

:::tabs
```js Node.js
const bots = new Map()
for (const nombre of ['principal', 'sub1', 'sub2']) {
  const bot = await SoblendWA.create({ store: { type: 'sqlite', path: 'bots.db', account: nombre }, mediaStore: { type: 'sqlite', path: 'media.db' } })
  bot.on('error', (error) => console.error(nombre, error))
  bots.set(nombre, bot)
  await bot.connect()
}

await bots.get('sub1').restart()
await bots.get('sub2').close()
```
```rust Rust
let bots = vec![("principal", "bots.db"), ("sub1", "bots.db")];
for (cuenta, archivo) in bots {
    let store = std::sync::Arc::new(soblendwa::store::SqliteStore::open_account(archivo, cuenta).await?);
    let (bot, eventos) = Client::builder().store(store).build().await?;
    bot.connect().await?;
}
```
:::

| Método | Qué hace con esa cuenta |
|---|---|
| `restart()` | La desconecta y la vuelve a conectar, sin tocar las demás |
| `disconnect()` | La desconecta y conserva la sesión |
| `close()` | La desconecta y quita sus handlers, para descartarla |
| `logout()` | La desvincula y borra su sesión |

En Node.js, un handler que lanza una excepción no corta los eventos de esa cuenta, y un handler `async` que falla no cierra el proceso. Los dos llegan al evento `error` de esa cuenta, o a la consola si no lo escuchas.

### Memoria por cuenta

Medido con `bindings/node/bench-accounts.js` contra el WhatsApp falso, en un solo proceso de Node.js, con todas las cuentas mandándose mensajes:

| Cuentas | RSS antes | RSS con las cuentas | Por cuenta | Una caída y otra cerrada |
|---|---|---|---|---|
| 20 | 33 MB | 65 MB | 1,6 MB | Las demás siguieron |
| 50 | 33 MB | 96 MB | 1,3 MB | Las demás siguieron |

Node 22 en Windows. El RSS incluye al servidor falso, que corre en el mismo proceso.

Es memoria de la librería con cuentas recién vinculadas. En producción crece con las cachés de cada cuenta: dispositivos, grupos, sesiones y el store de chats si lo activas. Mídelo en tu servidor con `node --expose-gc bench-accounts.js 20,50`.

## Tiempos por fase

Cada `Event::ServerAck` trae `timings`, para saber dónde se fue el tiempo de un envío.

| Campo | En JSON | Qué mide |
|---|---|---|
| `queued` | `queuedMicros` | Espera en la cola de su chat |
| `upload` | `uploadMicros` | Subida de multimedia, si la hubo |
| `prepare` | `prepareMicros` | Dispositivos y sesiones |
| `encrypt` | `encryptMicros` | Cifrado |
| `server` | `serverMicros` | Respuesta de WhatsApp |
| `total()` | `totalMicros` | La suma |

Un `ServerAck` con `error` trae el código de rechazo de WhatsApp. Uno sin `timings` suele ser una petición interna al teléfono.

Además, `stats().send_timings` resume los últimos 512 envíos confirmados, con red real, para que veas dónde se va el tiempo sin guardar cada evento:

```json
{
  "samples": 512,
  "queuedMicros": { "p50": 40, "p95": 310, "max": 2100 },
  "uploadMicros": { "p50": 380000, "p95": 1100000, "max": 2400000 },
  "prepareMicros": { "p50": 90, "p95": 120000, "max": 480000 },
  "encryptMicros": { "p50": 60, "p95": 900, "max": 4000 },
  "serverMicros": { "p50": 150000, "p95": 310000, "max": 900000 },
  "totalMicros": { "p50": 160000, "p95": 1300000, "max": 2600000 }
}
```

Los números de este ejemplo son ilustrativos. `uploadMicros` y `serverMicros` son `null` hasta que haya envíos con subida o con confirmación.

## Métricas {#confiabilidad-metricas}

:::tabs
```rust Rust
let stats = client.stats();
println!("{} recibidos, {} enviados", stats.received, stats.sent);
```
```js Node.js
const stats = bot.stats()
```
```python Python
stats = bot.stats()
```
```js Cliente JS
const stats = await bot.stats()
```
```bash HTTP
curl -X POST http://127.0.0.1:8787/api/bot/stats -H "Authorization: Bearer mi-clave"
```
:::

| Rust | JSON | Qué cuenta |
|---|---|---|
| `connected` | `connected` | Si está conectado ahora |
| `uptime` | `uptimeSecs` | Tiempo desde que arrancó |
| `received`, `sent` | `received`, `sent` | Mensajes recibidos y enviados |
| `undecryptable` | `undecryptable` | Los que no se pudieron descifrar |
| `recovered` | `recovered` | Los que se recuperaron con el reintento |
| `awaiting_recovery` | `awaitingRecovery` | Los que siguen esperando |
| `duplicates` | `duplicates` | Duplicados descartados |
| `reconnects` | `reconnects` | Reconexiones |
| `rate_limited` | `rateLimited` | Envíos frenados por el límite |
| `media_uploaded`, `media_reused` | `mediaUploaded`, `mediaReused` | Archivos subidos y reutilizados |
| `outbox_resent` | `outboxResent` | Reenvíos del outbox |
| `lost` | `lost` | Mensajes que no se recuperaron dentro del plazo |
| `send_timings` | `sendTimings` | Tiempos por fase de los últimos 512 envíos |
| `panics` | `panics` | Pánicos contenidos |
| `cached_devices`, `cached_groups` | `cachedDevices`, `cachedGroups` | Tamaño de las cachés |
| `pending_queries` | `pendingQueries` | Consultas esperando respuesta |
| `open_lanes` | `openLanes` | Colas de chat abiertas |
| `latency` | `latencyMicros` | Latencia de red |

### Prometheus

El servidor publica las métricas de todas sus cuentas en `GET /metrics`, en el formato de texto de Prometheus. Pide la misma clave que lo demás, en la cabecera o como `?token=`.

```yaml prometheus.yml
scrape_configs:
  - job_name: soblendwa
    metrics_path: /metrics
    authorization:
      credentials: mi-clave
    static_configs:
      - targets: ["127.0.0.1:8787"]
```

Cada serie lleva la etiqueta `session` con el nombre de la cuenta.

| Métrica | Tipo |
|---|---|
| `soblendwa_connected`, `soblendwa_uptime_seconds` | gauge |
| `soblendwa_messages_received_total`, `soblendwa_messages_sent_total` | counter |
| `soblendwa_messages_undecryptable_total`, `soblendwa_messages_recovered_total` | counter |
| `soblendwa_messages_lost`, `soblendwa_messages_awaiting_retry` | gauge |
| `soblendwa_messages_duplicate_total`, `soblendwa_reconnects_total`, `soblendwa_rate_limited_total` | counter |
| `soblendwa_media_uploaded_total`, `soblendwa_media_reused_total`, `soblendwa_outbox_resent_total`, `soblendwa_panics_total` | counter |
| `soblendwa_latency_seconds` | gauge |
| `soblendwa_send_phase_seconds{phase, quantile}` | gauge, cuantiles 0.5, 0.95 y 1 de cada fase |
| `soblendwa_process_resident_memory_bytes` | gauge, solo en Linux |

`lost` es gauge y no counter porque baja cuando un mensaje dado por perdido llega tarde.

```text Alerta de ejemplo
increase(soblendwa_messages_undecryptable_total[1h]) > 20
```

## Latencia {#confiabilidad-latencia}

```rust
let bot = msg.info.elapsed();
let red = client.latency();
let medido_ahora = client.ping().await?;
```

- `msg.info.elapsed()` mide cuánto lleva el mensaje dentro del bot, desde que llegó del socket. Incluye descifrarlo.
- `client.latency()` devuelve al instante la latencia de red medida en segundo plano.
- `client.ping()` hace una medición nueva y tarda un viaje de ida y vuelta.

La primera vez que el bot le escribe a un chat nuevo, tiene que pedir los dispositivos del destinatario y sus claves: uno o dos viajes de ida y vuelta, igual que WhatsApp Web. Después todo queda guardado y los envíos solo cuestan el cifrado local.

El tiempo que ves en el teléfono es la suma del viaje de tu teléfono a WhatsApp, unos 0,05 ms del bot, y el viaje de WhatsApp al bot y de vuelta. Esos viajes dependen de la distancia física y de tu conexión, y ningún código puede bajarlos de lo que permita la red.

=== rendimiento | Rendimiento | Operación
lead: Lo que se midió, cómo se midió y qué significa. Las pruebas son de laboratorio, y en producción manda la red.

## Un bot con base de datos, red y multimedia

El mismo bot escrito con cada librería, cada uno en su proceso, contra el mismo `@zapo-js/fake-server`:

- **Base de datos.** Los dos guardan todo en SQLite: SoblendWA con su `SqliteStore` y zapo con `@zapo-js/store-sqlite`.
- **Red.** Entre el bot y el servidor hay un proxy que retrasa cada paquete 30 ms ± 5, en orden: 60 ms de ida y vuelta con jitter, como una conexión normal.
- **Multimedia.** `!archivo` responde con un documento de 64 KB nuevo en cada pedido, así que ninguno se salva la subida con su caché.
- **Arranque.** Antes de medir, el bot se cierra y vuelve a abrir desde su base, sin vincular de nuevo.

Medido el 11 de noviembre de 2026 a las 17:00, con la PC sin otros programas pesados abiertos. Entre paréntesis, la peor y la mejor de las 5 corridas.

| Mediana de 5 corridas alternadas | SoblendWA {{version_label}} | zapo-js 1.11 |
|---|---|---|
| Privado, ida y vuelta p50 / p95 | **90 / 94 ms** | 168 / 186 ms |
| Grupo, ida y vuelta p50 | 91 ms | 92 ms |
| Documento de 64 KB, del pedido a recibirlo, p50 / p95 | **91 / 167 ms** | 169 / 187 ms |
| Ráfaga de 300 mensajes, respuestas por segundo | **1.018** (884 a 1.086) | 316 (302 a 348) |
| CPU del bot por mensaje, con SQLite | **0,8 ms** (0,7 a 1,6) | 5,2 ms (4,5 a 5,3) |
| Memoria al terminar | **59 MB** | 100 MB |
| Arrancar desde la base | **0,46 s** | 0,86 s |

:::note Cómo leer estos números
Una sola máquina Windows (Ryzen 5 7520U, Node.js 22), con el módulo de Node.js en release y zapo con su configuración por defecto. La memoria incluye a Node.js en los dos. SoblendWA solo confía en certificados reales, así que llega al servidor de multimedia falso por un puente HTTP local, un salto que zapo no tiene.

Las corridas anteriores, y por qué cambiaron:

| Corrida | Ráfaga, SoblendWA | Ráfaga, zapo | CPU por mensaje, zapo | Documento p95, SoblendWA | Documento p95, zapo |
|---|---|---|---|---|---|
| 10 de noviembre, 19:52 | 314 | 90 | 13,1 ms | 422 ms | 285 ms |
| 10 de noviembre, 21:58, con los hosts de subida pedidos al conectar | 422 | 84 | 12,8 ms | 298 ms | 258 ms |
| 10 de noviembre, 22:19, con el puente que ya no corta a los 5 s | 619 | 102 | 11,6 ms | 213 ms | 250 ms |
| 11 de noviembre, 17:00, con la PC libre; la de la tabla | 1.018 | 316 | 5,2 ms | 167 ms | 187 ms |

El p95 de documentos de SoblendWA bajó el 10 de noviembre por dos arreglos. El primer archivo esperaba un viaje más por los hosts de subida, que ahora se piden al conectar, y el puente cerraba las conexiones inactivas a los 5 s, lo que obligaba a reintentar.

Lo del 11 de noviembre no es un arreglo. zapo pasó de 102 a 316 respuestas por segundo y de 11,6 a 5,2 ms de CPU por mensaje sin que cambiara nada suyo: el 10 de noviembre la misma PC tenía otros programas pesados abiertos, y eso frenaba a los dos. Por eso la ráfaga y la CPU solo valen como comparación dentro de una misma corrida, nunca entre corridas.

El servidor falso y sus contactos corren en Node.js, en la misma máquina, y ponen un piso para los dos. Sigue sin ser WhatsApp de verdad: eso lo mide la [prueba en producción](#estado-produccion).
:::

```bash Terminal
npm ci --prefix tests/zapo
node tests/zapo/versus.cjs --rtt 60 --jitter 10 --runs 5
```

### Con internet de verdad de por medio {#rendimiento-internet}

El mismo bot, pero sin red simulada: con `--via usuario@host`, el chat de cada bot va de la PC a un VPS por un túnel SSH y vuelve a la PC, con la latencia y el jitter reales de internet. Medido el 11 de noviembre de 2026 a las 17:04, entre una PC en Colombia y un VPS, justo después de la tabla de arriba.

Cada sentido entre el bot y el servidor cruza internet dos veces, de la PC al VPS y de vuelta, así que cuesta lo mismo que un ping a ese VPS, unos 212 ms. Medida con eco sobre una conexión abierta por el mismo túnel, una ida y vuelta entre bot y servidor dio 392 ms, y ninguna respuesta puede bajar de ahí. SoblendWA contesta en 421 ms, unos 29 ms por encima de ese piso, como los 30 ms de la red simulada. zapo necesita dos idas y vueltas.

| Mediana de 3 corridas alternadas | SoblendWA {{version_label}} | zapo-js 1.11 |
|---|---|---|
| Privado, ida y vuelta p50 / p95 | **421 / 809 ms** | 830 / 916 ms |
| Grupo, ida y vuelta p50 | 414 ms | 422 ms |
| Documento de 64 KB, p50 / p95 | **441 / 828 ms** | 869 / 918 ms |
| Ráfaga de 300 mensajes, respuestas por segundo | **185** (166 a 185) | 75 (23 a 108) |
| CPU del bot por mensaje | **1,8 ms** | 4,9 ms |
| Memoria al terminar | **59 MB** | 100 MB |
| Arrancar desde la base | 2,24 s | **1,74 s** |

En grupo están parejos. Arrancar desde la base es lo único que zapo hace mejor con esta latencia: SoblendWA necesita unas idas y vueltas más antes de avisar `connected`, y con 392 ms cada una se nota; con la red simulada de 60 ms pasa al revés. El 10 de noviembre, con la PC cargada, había dado 2,4 contra 3,2 s. Lo que sigue sin ser real es el servidor: es el falso de zapo.

:::note Por qué antes decía 650 ms
La primera versión de esta sección decía "unos 650 ms de ida y vuelta". Con 458 ms de respuesta eso era imposible, y el número estaba mal medido: cronometraba una conexión nueva por el túnel, que además abre un canal SSH en cada punta, cuando los bots usan una conexión que ya está abierta. Ahora `versus.cjs --via` mide con eco sobre una conexión abierta por el mismo túnel, y el ping al VPS da lo mismo por otro camino.
:::

```bash Terminal
node tests/zapo/versus.cjs --via usuario@tu-vps --key ~/.ssh/tu-llave --runs 3
```

## Un bot de eco, sin red

Un bot de eco de punta a punta. Los dos se conectan al mismo `@zapo-js/fake-server` con el protocolo real (Noise, vinculación por QR, Signal), cada uno en su propio proceso y con almacenamiento en memoria. zapo corre con su acelerador `@zapo-js/native`.

| Mediana de 3 corridas alternadas (500 idas y vueltas + 500 en ráfaga) | SoblendWA | zapo-js 1.11 |
|---|---|---|
| Ida y vuelta, p50 / p95 / p99 | **4,2 / 10,1 / 16,3 ms** | 10,5 / 18,4 / 23,3 ms |
| Mensajes por segundo en ráfaga | **474** | 228 |
| CPU del bot por mensaje en ráfaga | **0,7 ms** | 4,5 ms |
| Memoria tras la prueba (pico) | **12 MB (24 MB)** | 130 MB (131 MB) |
| Conectar y vincular | **100 ms** | 1.565 ms |

:::note Cómo leer estos números
Se midió en una sola máquina Windows con otros programas abiertos, así que los valores absolutos varían entre corridas: la ráfaga de SoblendWA fue de 394 a 786 msg/s. Los tiempos incluyen al servidor falso, que corre en Node, así que marcan un piso para los dos. La memoria y la conexión de zapo incluyen el arranque de Node. Es un bot mínimo, sin multimedia, base de datos ni red real.
:::

## Las piezas, una por una

| | SoblendWA (Rust) | whatsmeow (Go) | Baileys 7 (JavaScript) |
|---|---|---|---|
| Codificar + decodificar un stanza de 21 nodos (3.375 bytes) | **63.242 stanzas/s** | 12.810 (4,9× menos) | 5.320 (11,9× menos) |
| Cifrar + descifrar un mensaje 1:1 (Signal) | **135.504 msg/s** | 12.223 (11× menos) | 3.720 (36× menos) |
| Cifrar + descifrar un mensaje de grupo (Sender Keys) | **9.117 msg/s** | 3.565 (2,6× menos) | 58 (157× menos) |

Se midió en la misma máquina contra el código real de cada librería: whatsmeow del 2 de noviembre de 2026 con su `libsignal` v0.2.2 (Go 1.27.2) y Baileys 7.0.0-rc14 con su `libsignal` y su `GroupCipher`. Mismo stanza, mismo mensaje de 120 bytes y mismas operaciones; las tres librerías producen exactamente los mismos 3.375 bytes al codificar. SoblendWA serializa la sesión en cada mensaje; a whatsmeow y Baileys se les midió con los registros como objetos en memoria, que es su caso más rápido. whatsmeow serializando como su almacén SQL baja a 6.773 msg/s.

Puedes repetir lo nuestro:

```bash Terminal
cargo test --release -- --ignored throughput
cargo test --release --features testing --lib -- --ignored pipeline --nocapture
```

## Desde Node.js

Dos cuentas en el mismo proceso de Node.js contra el WhatsApp falso: una manda 10.000 mensajes seguidos a la otra sin esperar cada uno. Misma máquina y mismas opciones de compilación, con la versión publicada antes de esta y con esta.

| | 0.1.0 (anterior) | {{version_label}} |
|---|---|---|
| Envíos por segundo desde JavaScript | ~7.500 | ~9.000 a 9.900, y en orden |
| Mensajes que llegan al handler por segundo | ~4.600 | ~6.500 |
| Un mensaje convertido a JSON | ~18,7 µs | ~3 µs |
| Cargar una sesión Signal guardada | ~54 µs | ~4 µs |

En la versión anterior los envíos al mismo chat sin `await` podían salir desordenados; ahora salen en el orden en que se llamaron, y eso cuesta algo de velocidad cuando todos van al mismo chat. Entre chats distintos no hay espera. En un bot real, WhatsApp limita mucho antes de llegar a estas cifras.

## Por qué es rápido

Desde que llega un mensaje hasta que sale la respuesta, el bot tarda unos **0,05 ms** en un chat conocido.

- Sin esperas de red en chats conocidos. Las sesiones, los dispositivos y los datos de los grupos se guardan en disco y se usan al instante, aunque reinicies el bot. Se actualizan en segundo plano y cuando WhatsApp avisa de un cambio. Al conectar se precargan tus grupos y los dispositivos de sus miembros.
- Grupos grandes rápidos desde el primer mensaje. Las consultas de claves y dispositivos salen en paralelo, en un solo viaje de red, y las sesiones Signal se crean usando todos los núcleos: 1000 dispositivos en unos 0,1 s en lugar de 0,4 s.
- Socket sin demoras. `TCP_NODELAY` evita que el sistema retenga mensajes pequeños. El lector y el escritor tienen cada uno su propio cifrador AES-GCM y no comparten locks.
- Arranque rápido. La versión de WhatsApp Web se recuerda y se actualiza en segundo plano.
- Almacenamiento sin bloqueos. `FileStore` escribe en un log de solo-escritura únicamente lo que cambió y compacta de vez en cuando. Los backends SQL y Redis agrupan las escrituras en segundo plano.
- Colas por chat. Los chats distintos van en paralelo y cada chat conserva su orden.
- Multimedia reutilizada. El mismo archivo no se vuelve a cifrar ni a subir.
- Eventos en lotes. Node.js y Python reciben todos los eventos que estén listos en una sola llamada, como un solo texto JSON, en vez de una promesa por evento.
- JSON escrito directo. Los mensajes, recibos y confirmaciones se escriben como texto sin pasar por un árbol de objetos intermedio.
- Menos reservas de memoria. Los módulos de Node.js y Python y el servidor usan mimalloc, y los JID se formatean sin la maquinaria de `format!`.
- Sesiones que cargan al instante. La clave pública del ratchet se guarda junto a la privada, así que leer una sesión no repite una multiplicación de curva. Al firmar en grupos, la clave de Edwards se calcula una vez por sender key.
- Turnos por chat. Cada envío toma su lugar al llamarse y, al terminar, despierta solo al siguiente de ese chat.
- Sin código inseguro. `#![forbid(unsafe_code)]`, y el decodificador binario está probado contra cientos de miles de entradas hostiles.

=== almacenamiento | Almacenamiento | Operación
lead: Dónde se guardan las claves y el estado de cada cuenta.

## Elegir un store

| Store | Feature | Para qué | Estado |
|---|---|---|---|
| `FileStore` | incluido | Un bot en una máquina. Log de solo-escritura con compactación | {ok} |
| `MemoryStore` | incluido | Pruebas. Se pierde todo al salir | {ok} |
| `SqliteStore` | `sqlite` | Un archivo, varias cuentas, sin servidor. SQLite va incluido en el binario | {ok} |
| `PostgresStore` | `postgres` | Muchos bots o servidores compartiendo una base | {exp} |
| `RedisStore` | `redis` | Contenedores efímeros con estado en Redis | {exp} |
| `MongoStore` | `mongo` | Bots que ya usan MongoDB | {ok} |
| Tu propio store | — | Cualquier base de datos, en Rust o en JavaScript | {ok} |

Los tres pasan contra servidores reales en el CI (MongoDB 7, PostgreSQL 16 y Redis 7), `store-check.js` incluido. `MongoStore` además recibe en el CI una sesión que escribió zapo con su propio store de Mongo y sigue con ella las mismas conversaciones ([cómo](#migrar-zapo-ida-y-vuelta)). PostgreSQL y Redis siguen marcados como experimentales porque todavía nadie los ha usado semanas con datos de producción.

## Abrirlos

:::tabs
```rust Rust
use std::sync::Arc;
use soblendwa::store::{MongoStore, PostgresStore, RedisStore, SqliteStore};
use soblendwa::{FileStore, MemoryStore};

let archivos = FileStore::open("session").await?;
let memoria = Arc::new(MemoryStore::new());
let sqlite = Arc::new(SqliteStore::open_account("datos/bots.db", "ventas").await?);
let postgres = Arc::new(PostgresStore::connect_account("postgres://bot:clave@localhost/wa", "ventas").await?);
let redis = Arc::new(RedisStore::connect_account("redis://127.0.0.1/", "ventas").await?);
let mongo = Arc::new(MongoStore::connect_account("mongodb://localhost:27017", "wa", "ventas").await?);

let (client, events) = Client::builder().store(sqlite).build().await?;
```
```js Node.js
const opciones = [
  { type: 'file', path: 'session' },
  { type: 'memory' },
  { type: 'sqlite', path: 'datos/bots.db', account: 'ventas' },
  { type: 'postgres', url: 'postgres://bot:clave@localhost/wa', account: 'ventas' },
  { type: 'redis', url: 'redis://127.0.0.1/', account: 'ventas' },
  { type: 'mongo', url: 'mongodb://localhost:27017', database: 'wa', account: 'ventas' },
]
const bot = await SoblendWA.create({ store: opciones[0] })
```
```python Python
bot = await SoblendWA.create(store={"type": "sqlite", "path": "datos/bots.db", "account": "ventas"})
```
```bash Servidor
soblendwa-server --dir sessions
soblendwa-server --sqlite bots.db
soblendwa-server --memory
soblendwa-server --mongo mongodb://localhost:27017 --mongo-db wa
```
:::

`account` permite guardar varias cuentas en la misma base. Con `FileStore`, usa una carpeta por cuenta. Si no indicas un store, Rust usa memoria, y Node.js y Python usan la carpeta `session`.

Las credenciales y las claves de la cuenta son secretos. Protege el acceso a tu base de datos y a tu carpeta de sesión como protegerías una contraseña.

## Cómo funciona por dentro

Todos los backends de Rust comparten el mismo núcleo, `BatchedStore`: las lecturas salen de memoria después de la primera vez, y las escrituras se agrupan en una sola transacción, o un solo pipeline en Redis, en segundo plano. Si una clave cambia mil veces antes de escribirse, solo se escribe el último valor. Enviar un mensaje nunca espera a la base de datos. `client.disconnect()` vacía lo pendiente antes de salir.

## Multimedia compartida entre cuentas {#almacenamiento-multimedia-compartida}

Por defecto, cada cuenta recuerda lo que subió en su propio store. Con un store de multimedia compartido, todas tus cuentas reutilizan lo que ya subió cualquiera de ellas, y lo recuerdan tras reiniciar. Si tus subbots envían la misma imagen de un comando, solo la primera la sube.

:::tabs
```js Node.js
const compartido = { type: 'sqlite', path: 'media.db' }
const principal = await SoblendWA.create({ store: { type: 'sqlite', path: 'bots.db', account: 'principal' }, mediaStore: compartido })
const sub = await SoblendWA.create({ store: { type: 'sqlite', path: 'bots.db', account: 'sub1' }, mediaStore: compartido })
```
```python Python
compartido = {"type": "sqlite", "path": "media.db"}
principal = await SoblendWA.create(store={"type": "sqlite", "path": "bots.db", "account": "principal"}, media_store=compartido)
```
```rust Rust
use std::sync::Arc;
use soblendwa::store::SqliteStore;

let compartido: Arc<dyn soblendwa::Store> = Arc::new(SqliteStore::open("media.db").await?);
let (principal, eventos) = Client::builder().store(store).media_store(compartido.clone()).build().await?;
```
:::

En Node.js y Python, las cuentas del mismo proceso que pasan las mismas opciones comparten una sola instancia del store. Entre procesos distintos, cada uno guarda su propia caché en memoria, aunque el archivo sea el mismo. Probado contra el WhatsApp falso: la segunda cuenta envía la imagen sin volver a subirla. Falta confirmarlo con cuentas reales. {exp}

## Store escrito en JavaScript

Con el módulo de Node.js pasas un objeto con tus funciones y usas la base de datos que quieras. Las lecturas ya pasan por caché y las escrituras llegan agrupadas, así que no hace falta optimizar nada.

```js
const store = {
  loadCreds: () => db.creds.findOne(),
  saveCreds: (json) => db.creds.save(json),
  get: (kind, key) => db.keys.get(`${kind}:${key}`),
  keys: (kind) => db.keys.list(kind),
  apply: (writes) => db.keys.bulk(writes),
}
const bot = await SoblendWA.create({ store })
```

| Función | Recibe | Devuelve |
|---|---|---|
| `loadCreds()` | — | El texto JSON guardado, o `null` |
| `saveCreds(json)` | Un texto JSON | — |
| `get(kind, key)` | Tipo y clave | Un `Buffer`, un `Uint8Array` o `null` |
| `keys(kind)` | Tipo | Lista de claves. Opcional, se usa al exportar a zapo |
| `apply(writes)` | Lista de `{ op, kind, key, value }`, con `op` igual a `set`, `remove` o `clear` | — |

En lugar de `apply` puedes dar `set(kind, key, value)`, `remove(kind, key)` y `clear()`. Probado de punta a punta: vinculación, mensajes y reinicio con el mismo store.

## Store escrito en Rust

Implementa `Backend`, con cuatro métodos más `list` si quieres exportar a zapo, y envuélvelo en `BatchedStore::new(tu_backend)`: la caché y el agrupado vienen gratis.

```rust
use async_trait::async_trait;
use soblendwa::store::{Creds, KeyKind, KeyWrite};
use soblendwa::{Backend, Result};

struct MiBaseDeDatos;

#[async_trait]
impl Backend for MiBaseDeDatos {
    async fn load_creds(&self) -> Result<Option<Creds>> {
        Ok(None)
    }

    async fn save_creds(&self, _creds: &Creds) -> Result<()> {
        Ok(())
    }

    async fn fetch(&self, _kind: KeyKind, _key: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    async fn apply(&self, _writes: &[KeyWrite]) -> Result<()> {
        Ok(())
    }
}
```

Si prefieres controlarlo todo, implementa directamente el trait `Store`, con `load_creds`, `save_creds`, `get`, `set`, `remove`, `clear`, y opcionalmente `flush` y `keys`.

## Qué se guarda

`KeyKind` indica el tipo de cada dato.

| `KeyKind` | Qué guarda |
|---|---|
| `Session` | Sesiones Signal con cada dispositivo |
| `PreKey` | Tus pre-keys |
| `SenderKey`, `SenderKeyMemory` | Claves de grupo y a quién se le enviaron |
| `LidMapping` | Pares número ↔ LID |
| `DeviceList` | Dispositivos de cada cuenta |
| `GroupMetadata` | Datos de los grupos |
| `PollSecret` | Secretos para descifrar votos |
| `AppStateKey`, `AppStateVersion` | Claves y versiones del app state |
| `TcToken` | Tokens de privacidad |
| `MediaCache` | Multimedia reutilizable |
| `Outbox` | Mensajes sin confirmar |
| `RecentSend` | Envíos recientes, para atender reintentos |

=== configuracion | Configuración | Operación
lead: Todas las opciones del cliente, con sus valores por defecto.

```rust
use std::time::Duration;
use soblendwa::{Client, Platform, SendLimit};

let (client, events) = Client::builder()
    .store(store)
    .device_name("SoblendWA")
    .platform(Platform::Desktop)
    .push_name("Mi Bot")
    .online_on_connect(false)
    .auto_reconnect(true)
    .query_timeout(Duration::from_secs(30))
    .send_limit(Some(SendLimit { per_chat_per_minute: 20, per_chat_burst: 8, ..Default::default() }))
    .outbox(true)
    .media_cache(Some(Duration::from_secs(7 * 24 * 60 * 60)))
    .workers(16)
    .build()
    .await?;
```

| Método | Por defecto | Para qué |
|---|---|---|
| `store(store)` | Memoria | Dónde se guarda la sesión |
| `chat_store(chats)` | Ninguno | Activa el [store de chats](#store-chats) |
| `device_name(texto)` | `SoblendWA` | Nombre en **Dispositivos vinculados** |
| `platform(plataforma)` | `Chrome` | `Chrome`, `Firefox`, `Safari`, `Edge`, `Opera` o `Desktop` |
| `push_name(texto)` | Ninguno | Tu nombre visible |
| `country_code(texto)` | `US` | País que se le informa a WhatsApp |
| `version([a, b, c])` | Automática | Fija la versión de WhatsApp Web, en lugar de la más reciente |
| `online_on_connect(bool)` | `true` | Aparecer en línea al conectar |
| `auto_reconnect(bool)` | `true` | Reconectar solo |
| `query_timeout(duración)` | 60 s | Tiempo máximo para una consulta |
| `connect_timeout(duración)` | 20 s | Tiempo máximo para conectar |
| `keepalive(duración)` | 15 s | Cada cuánto se comprueba que la conexión sigue viva |
| `send_limit(límite)` | 240/min, 40 por chat | [Límite de envíos](#confiabilidad-limite) |
| `outbox(bool)` | `true` | [Outbox persistente](#confiabilidad-outbox) |
| `media_cache(duración)` | 7 días | [Reutilización de multimedia](#multimedia-reutilizacion-automatica) |
| `link_previews(config)` | Activadas | [Vista previa de enlaces](#vista-previa) |
| `proxy(proxy)` | Ninguno | [Proxy](#proxy) |
| `workers(número)` | Núcleos × 2, entre 4 y 64 | Mensajes procesados en paralelo |
| `media_store(store)` | El de la cuenta | [Multimedia compartida](#almacenamiento-multimedia-compartida) entre cuentas |
| `retry_deadline(duración)` | 10 minutos | Cuánto esperar un mensaje que no se pudo descifrar antes de darlo por perdido |
| `config(config)` | — | Reemplaza toda la configuración de una vez |
| `ws_url(url)` | WhatsApp real | Conectar a otro servidor, como uno falso |
| `trust_root(clave)` | La de WhatsApp | Raíz de los certificados de ese servidor |
| `media_endpoint(origen)` | Automático | Servidor de multimedia alternativo |

:::warn Déjalos como están en producción
`ws_url`, `trust_root` y `media_endpoint` existen para probar contra servidores falsos. `trust_root` cambia en quién confía el cliente, así que no lo uses con un servidor en el que no confíes.
:::

## En Node.js, Python y el servidor

Todas las opciones de Rust existen en los demás lenguajes. Se pasan a `SoblendWA.create`; en Python usan snake_case y se convierten solas. En el servidor van en `--options` como JSON (o `--options @opciones.json`) y valen para todas sus cuentas.

| JavaScript | Python | Rust |
|---|---|---|
| `store` | `store` | `store` |
| `chatStore: { path, limit }` | `chat_store={...}` | `chat_store` |
| `pushName` | `push_name` | `push_name` |
| `deviceName` | `device_name` | `device_name` |
| `platform` | `platform` | `platform` |
| `countryCode` | `country_code` | `country_code` |
| `onlineOnConnect` | `online_on_connect` | `online_on_connect` |
| `autoReconnect` | `auto_reconnect` | `auto_reconnect` |
| `proxy` | `proxy` | `proxy` |
| `linkPreviews` | `link_previews` | `link_previews` |
| `server: { wsUrl, noiseRootHex, mediaEndpoint }` | `server={...}` | `ws_url`, `trust_root`, `media_endpoint` |
| `mediaStore` | `media_store` | `media_store` |
| `retryDeadlineSecs` | `retry_deadline_secs` | `retry_deadline` |
| `sendLimit` | `send_limit` | `send_limit` |
| `outbox` | `outbox` | `outbox` |
| `mediaCache: false` o `{ ttlSecs }` | `media_cache=...` | `media_cache` |
| `workers` | `workers` | `workers` |
| `queryTimeoutSecs` | `query_timeout_secs` | `query_timeout` |
| `connectTimeoutSecs` | `connect_timeout_secs` | `connect_timeout` |
| `keepaliveSecs` | `keepalive_secs` | `keepalive` |
| `version: '2.3000.1052682108'` | `version=...` | `version` |

```js Node.js
const bot = await SoblendWA.create({
  store: { type: 'sqlite', path: 'session/auth.db' },
  workers: 16,
  queryTimeoutSecs: 30,
  mediaCache: { ttlSecs: 3 * 24 * 60 * 60 },
  outbox: true,
})
```

Una opción mal escrita no se ignora: falla con `INVALID_ARGUMENT` y te dice cuál quisiste poner, por ejemplo `unknown option "sendLimt", did you mean "sendLimit"?`. Una prueba del proyecto compara esta tabla con la configuración de Rust, así que una opción nueva no puede quedarse solo en Rust.

## Registros

SoblendWA escribe sus registros con `tracing`. En Rust, instala un suscriptor como `tracing-subscriber`. El servidor los muestra por defecto, y se ajustan con la variable `RUST_LOG`.

```bash Terminal
RUST_LOG=soblendwa=debug,soblendwa_server=info soblendwa-server
```

=== proxy | Proxy | Operación
lead: Pasa toda la conexión por un proxy HTTP o SOCKS5.

:::tabs
```rust Rust
use soblendwa::Proxy;

let proxy = Proxy::parse("socks5://usuario:clave@127.0.0.1:1080")?;
let (client, events) = Client::builder().store(store).proxy(proxy).build().await?;
```
```js Node.js
const bot = await SoblendWA.create({ proxy: 'socks5://usuario:clave@127.0.0.1:1080' })
```
```python Python
bot = await SoblendWA.create(proxy="socks5://usuario:clave@127.0.0.1:1080")
```
```bash Servidor
soblendwa-server --proxy socks5://usuario:clave@127.0.0.1:1080
```
:::

| Esquema | Qué es |
|---|---|
| `http://` | Proxy HTTP con CONNECT |
| `socks5://` | SOCKS5 |
| `socks5h://` | SOCKS5 resolviendo los nombres en el proxy |

El usuario y la contraseña son opcionales. Pasan por el proxy el WebSocket, las subidas y descargas de multimedia, las vistas previas de enlaces y la consulta de la versión de WhatsApp Web. Se probó contra los servidores reales de WhatsApp a través de un SOCKS5 con usuario y contraseña.

=== migrar | Migrar desde otra librería | Operación
lead: Trae tu sesión de zapo o de Baileys sin volver a escanear el QR.

:::warn Antes de migrar
Detén el bot anterior. Dos procesos no pueden usar la misma sesión a la vez. Haz también una copia de tu base o carpeta de sesión.
:::

## Desde zapo con SQLite

```bash Terminal
cargo run --release --example import_zapo --features sqlite -- ./auth/state.sqlite bot ./session
```

Convierte las credenciales, las sesiones Signal, incluidas las claves de mensajes que llegaron fuera de orden, las pre-keys, las sender keys de grupos, las claves y versiones de app state y los pares número ↔ LID de los contactos. La base de zapo se abre **solo para leer**: no se modifica.

### Volver a zapo

Si algo no te convence, escribe el estado actual de SoblendWA de vuelta en la base de zapo y sigue con tu bot de antes, con los ratchets al día.

```bash Terminal
cargo run --release --example import_zapo --features sqlite -- --rollback ./session ./auth/state.sqlite bot
```

Probado contra zapo-js 1.11 real, en los dos sentidos: zapo crea la base, SoblendWA la importa, descifra los mensajes pendientes y responde; los contactos de zapo descifran esas respuestas; tras el rollback, zapo sigue mandando y recibiendo mensajes privados y de grupo.

### Desde tu código

```rust
use soblendwa::store::{export_zapo_sqlite, import_zapo_sqlite, zapo_sessions};

let sesiones = zapo_sessions("auth/state.sqlite")?;
let destino = FileStore::open("session").await?;
let reporte = import_zapo_sqlite("auth/state.sqlite", "bot", destino.as_ref()).await?;
println!("{} sesiones, {} pre-keys", reporte.sessions, reporte.pre_keys);
export_zapo_sqlite(destino.as_ref(), "auth/state.sqlite", "bot").await?;
```

## Desde zapo sin instalar Rust {#migrar-desde-node}

El módulo de Node.js trae la migración, así que la puedes correr en tu VPS sin compilar nada. Funciona con las bases SQLite y MongoDB de zapo.

```js
const { importZapo, rollbackZapo } = require('soblendwa-node')

const zapo = { type: 'mongo', url: 'mongodb://localhost:27017', database: 'bot_copia', session: 'principal' }

const prueba = await importZapo(zapo, null, { check: true })
console.log(prueba)

const real = await importZapo(zapo, { type: 'mongo', url: 'mongodb://localhost:27017', database: 'soblendwa', account: 'principal' })

await rollbackZapo({ type: 'mongo', url: 'mongodb://localhost:27017', database: 'soblendwa', account: 'principal' }, zapo)
```

| Función | Qué hace | ¿Escribe? |
|---|---|---|
| `importZapo(zapo, null, { check: true })` | Lee la sesión de zapo, la convierte en memoria y te da el reporte | No escribe nada |
| `importZapo(zapo, destino)` | Convierte la sesión y la guarda en el store de destino, que debe estar vacío | Solo en el destino |
| `rollbackZapo(origen, zapo)` | Escribe el estado de SoblendWA de vuelta en la base de zapo | Sí, en la base de zapo |

Si la base tiene una sola sesión, `session` es opcional. Si tus colecciones usan un prefijo, pásalo como `collectionPrefix`. Revisa siempre `failed` en el reporte.

### Probar con una copia de tu base {#migrar-copia}

Antes de tocar tus sesiones de verdad, hazlo con una copia. Así nadie tiene que volver a vincular si algo falla.

:::steps
- Copia la base de zapo: `mongodump --uri "mongodb://…" --db bot --archive=bot.archive` y `mongorestore --uri "mongodb://…" --archive=bot.archive --nsFrom "bot.*" --nsTo "bot_copia.*"`.
- Corre `importZapo` con `{ check: true }` sobre `bot_copia` y revisa que `failed` esté vacío y que los números de sesiones y claves tengan sentido.
- Detén el subbot que vas a probar en zapo. Nunca uses una sesión en dos procesos a la vez.
- Importa esa sesión a un store vacío de SoblendWA y arranca el subbot con SoblendWA. Prueba privados, grupos, multimedia y comandos.
- Para volver, detén SoblendWA y corre `rollbackZapo` hacia la base desde la que arrancará zapo. Arranca zapo y comprueba que sigue enviando y recibiendo.
- Cuando todo funcione, repite con la base real, cuenta por cuenta.
:::

### Simulacro con una base de producción {#migrar-simulacro}

`tests/zapo/dry-run.cjs` hace el simulacro sobre todas las sesiones de una base, sin conectar nada: para cada prefijo y cada sesión corre el `check`, una importación completa a un SQLite temporal, y la comprobación de vinculación: que al abrir lo importado, SoblendWA vea una cuenta ya vinculada, con su número y sus credenciales. No la conecta, así que no prueba que WhatsApp la acepte. Solo imprime conteos y tiempos.

```bash Terminal
mongodump --db zapo --archive | mongorestore --archive --nsInclude "zapo.wa_*" --nsFrom "zapo.*" --nsTo "zapo_copia.*"
node tests/zapo/dry-run.cjs mongodb://127.0.0.1:27017 zapo_copia
```

El `--nsInclude` copia solo las colecciones de zapo y deja fuera los datos de tu bot. Así salió con la base de un bot real en producción, el 10 de noviembre de 2026, un bot principal y sus subbots con un prefijo cada uno:

| | |
|---|---|
| Colecciones de zapo | 573, en 68 prefijos |
| Sesiones encontradas | 27, de las cuales 5 vinculadas y 22 que nunca terminaron de vincularse |
| Importadas sin error | 27 de 27, con 0 elementos que no se pudieron convertir |
| Convertido | 2.780 sesiones Signal, 2.899 sender keys y 3.484 pre-keys, los mismos conteos que los documentos de la copia |
| Pasaron la comprobación de vinculación | Las 5 vinculadas. Ninguna se conectó todavía con SoblendWA |
| Tiempo | 1,5 s en total; la cuenta más grande, 148 ms |

Dos cosas de zapo no se convierten. `sender_key_distribution` dice a qué dispositivos ya les llegó tu clave de grupo; sin ella, tu primer mensaje en cada grupo después de migrar vuelve a repartirla a todos, una sola vez. Y `privacy_tokens`, que WhatsApp vuelve a emitir con el uso. Ninguna de las dos impide enviar ni recibir.

Conectar una cuenta importada sí exige apagarla antes en zapo: una misma sesión en dos procesos se pisa.

## Desde zapo con MongoDB

```bash Terminal
cargo run --release --example import_zapo --features "sqlite mongo" -- --mongo mongodb://localhost:27017 mi_base bot ./session
cargo run --release --example import_zapo --features "sqlite mongo" -- --rollback-mongo ./session mongodb://localhost:27017 mi_base bot
```

Desde Node.js es `importZapo({ type: 'mongo', url, database, session })`, con `{ check: true }` para un simulacro que solo lee, y `rollbackZapo(store, { type: 'mongo', … })` para volver. Desde Rust: `ZapoMongo::connect`, `import_zapo_mongo` y `export_zapo_mongo`. Si usas un prefijo de colecciones, indícalo con `.collection_prefix(...)` o `collectionPrefix`.

### Cómo se prueba {#migrar-zapo-ida-y-vuelta}

`tests/zapo/round-trip.cjs` hace la migración completa con el zapo de verdad, sin datos armados a mano:

1. zapo 1.11 vincula un bot contra `@zapo-js/fake-server` y guarda todo con su propio `@zapo-js/store-mongo` (o `@zapo-js/store-sqlite`).
2. Dos contactos le escriben en privado y en un grupo, y el bot les contesta.
3. SoblendWA hace el simulacro, importa, se conecta sin volver a vincular y sigue hablando con los mismos contactos: el privado sale como `msg`, es decir, el mismo ratchet que dejó zapo y no una sesión nueva, y el grupo usa las sender keys de zapo.
4. `rollbackZapo` devuelve el estado a la base de zapo, zapo arranca desde ahí y la conversación sigue.

El CI la corre en cada push contra MongoDB 7 y con SQLite. Para correrla tú:

```bash Terminal
npm ci --prefix tests/zapo
node tests/zapo/round-trip.cjs mongo mongodb://127.0.0.1:27017
```

Antes de migrar tu bot de producción, haz el simulacro y una copia de la base.

## Desde Baileys

:::tabs
```bash Terminal
cargo run --release --example import_baileys -- ./auth_info ./session
```
```rust Rust
let store = FileStore::open("session").await?;
let reporte = soblendwa::store::import_baileys("auth_info", store.as_ref()).await?;
println!("{} sesiones, {} claves de grupo", reporte.sessions, reporte.sender_keys);
```
```js Node.js
const { importBaileys } = require('soblendwa-node')
const report = await importBaileys('./auth_info', { type: 'sqlite', path: 'session/auth.db' })
```
```python Python
from soblendwa import import_baileys

report = await import_baileys("./auth_info", type="sqlite", path="session/auth.db")
```
:::

Convierte las credenciales, las sesiones Signal de libsignal-node, las pre-keys, las sender keys de grupos, las claves y versiones de app state y los mapeos número ↔ LID. Las listas de dispositivos y los tokens se vuelven a pedir solos. El bot sigue vinculado, y los chats en curso siguen cifrándose con las mismas sesiones.

El reporte trae `me`, `sessions`, `pre_keys`, `sender_keys`, `app_state_keys`, `app_state_versions`, `lid_mappings`, `skipped` y `failed`. Revisa `failed` por si algo no se pudo convertir.

## Equivalencias con Baileys

Cada función pública de Baileys 7 tiene su equivalente.

| Baileys | SoblendWA |
|---|---|
| `useMultiFileAuthState` | `FileStore`, e `import_baileys` para traer tu sesión |
| `makeInMemoryStore` | `ChatStore` |
| `sendMessage({ text })`, `{ image }`, `{ video }`… | `send_text`, `send_image`, `send_media_file`, `prepare_media` + `send_message` |
| `{ ptv: true }` | `Message::video_note` |
| `{ contacts }`, `{ location }` | `Message::contacts`, `Message::vcard`, `Message::location` |
| `{ react }`, `{ delete }`, `{ edit }`, `{ forward }` | `react`, `revoke_message`, `edit_message`, `.forwarded()` |
| `{ poll }`, `{ event }`, `{ album }`, `{ pin }` | `Message::poll` / `announcement_poll`, `Message::event(..).event_details(..)`, `send_album`, `pin_message` |
| `{ groupInvite }`, `{ product }` | `send_group_invite`, `send_product` |
| `{ buttonReply }`, `{ listReply }` | `Message::button_reply`, `template_reply`, `list_reply` |
| `{ sharePhoneNumber }`, `{ requestPhoneNumber }`, `{ limitSharing }` | `Message::share_phone_number`, `request_phone_number`, `limit_sharing` |
| `mentions`, `mentionAll` | `.mention(..)`, `.mention_all()` |
| `readMessages`, `sendReceipt(s)` | `read_message`, `mark_read`, `mark_played`, `send_receipt` |
| `chatModify`, `star`, `addOrEditContact`, `removeContact` | `archive_chat`, `pin_chat`, `mute_chat`, `mark_chat_read`, `delete_chat`, `clear_chat`, `star_message`, `save_contact`, `chat_modify` |
| `addLabel`, `addChatLabel`, `addMessageLabel` (y sus `remove`) | `edit_label`, `delete_label`, `label_chat`, `label_message` |
| `addOrEditQuickReply` | `save_quick_reply`, `delete_quick_reply` |
| `update*Privacy`, `updateDisableLinkPreviewsPrivacy` | `set_privacy`, `set_link_previews_disabled` |
| `updateDefaultDisappearingMode`, `disappearingMessagesInChat`, `fetchDisappearingDuration` | `set_default_disappearing`, `set_disappearing`, `disappearing_modes` |
| `fetchStatus`, `updateProfileStatus`, `updateProfileName` | `fetch_about`, `set_about`, `set_push_name` |
| `profilePictureUrl`, `updateProfilePicture`, `removeProfilePicture` | `profile_picture_url`, `set_profile_picture`, `remove_profile_picture` |
| `updateBlockStatus`, `fetchBlocklist`, `fetchPrivacySettings` | `block`, `unblock`, `blocklist`, `privacy_settings` |
| `onWhatsApp`, `getBusinessProfile` | `on_whatsapp`, `business_profile` |
| `group*` | `group_info`, `group_metadata`, `create_group`, `update_participants`, `set_group_*`, `group_invite_code`, `join_group`, `accept_group_invite`, `group_join_requests`… |
| `community*` | `create_community`, `create_community_group`, `link_group`, `unlink_group`, `community_groups`, `leave_community`, y las funciones de grupo |
| `newsletter*`, `subscribeNewsletterUpdates` | `newsletter_*`, `newsletter_live_updates` |
| `getCatalog`, `getCollections`, `getOrderDetails`, `product*`, `updateBussinesProfile`, `updateCoverPhoto` | `catalog`, `catalog_collections`, `order_details`, `product_*`, `update_business_profile`, `set_business_cover_photo` |
| `updateMemberLabel`, `getBotListV2` | `set_member_label`, `bot_list` |
| `fetchAccountReachoutTimelock`, `fetchNewChatMessageCap` | `reachout_timelock`, `new_chat_message_cap` |
| `fetchMessageHistory`, `requestPlaceholderResend`, `updateMediaMessage` | `request_history`, `request_placeholder_resend`, `reupload_media` |
| `rejectCall`, `createCallLink`, `sendPresenceUpdate`, `presenceSubscribe` | `reject_call`, `create_call_link`, `send_chat_presence` / `set_presence`, `subscribe_presence` |
| `requestPairingCode`, `logout`, `end`, `cleanDirtyBits`, `rotateSignedPreKey` | `request_pair_code`, `logout`, `disconnect`, `clean_dirty_bits`, `rotate_signed_pre_key` |
| `relayMessage`, `sendNode`, `query` | `send_message_with`, `send_node`, `query` |

## Equivalencias con zapo {#migrar-equivalencias-zapo}

Sacadas de zapo-js 1.11. La columna de SoblendWA es la de Node.js; en Python son los mismos nombres en snake_case, y en Rust están en la [referencia](#ref-client).

| zapo-js | SoblendWA (Node.js) |
|---|---|
| `new WaClient({ store, sessionId })` con `createStore({ backends, providers })` | `SoblendWA.create({ store })`; para seguir con tu base de zapo, [`importZapo`](#migrar) una vez |
| `createSqliteStore`, `createMongoStore`, `createPostgresStore`, `createRedisStore` | `store: { type: 'sqlite' \| 'mongo' \| 'postgres' \| 'redis', … }`, o un [store en JavaScript](#almacenamiento-store-escrito-en-javascript) |
| `collectionPrefix` de `@zapo-js/store-mongo`, un prefijo por subbot | `account` del store: varias cuentas en la misma base |
| `client.connect()`, `client.disconnect()`, `client.logout()` | `bot.connect()`, `bot.close()`, `bot.logout()` |
| `auth.requestPairingCode(phone)` | `bot.requestPairCode(phone)` |
| `on('auth_qr')`, `on('auth_paired')` | `on('qr')`, `on('pairSuccess')` |
| `on('connection')` con `status: 'open' \| 'close'` | `on('connected')`, `on('disconnected')`; cierre definitivo en `on('loggedOut')` con su [causa](#vincular-causa) |
| `on('message')`, `event.key`, `event.message` | `on('message')`, `message.info`, `message.text`, `message.media` |
| `on('message_unavailable')` | `on('undecryptableMessage')`, y después `messageRecovered` o `messageLost` |
| `on('receipt')`, `on('presence')`, `on('chatstate')`, `on('call')` | `on('receipt')`, `on('presence')`, `on('chatState')`, `on('call')` |
| `on('group')` | `on('groupUpdate')`, más `joinedGroup` / `leftGroup` cuando entra o sale el bot |
| `on('picture')`, `on('privacy')`, `on('blocklist')` | `on('pictureChanged')`, `on('privacyChanged')`, `on('blocklistChanged')` |
| `on('history_sync_chunk')` | `on('historySync')` |
| `message.send(jid, 'texto')` | `bot.sendText(chat, 'texto')` |
| `message.send(jid, { type: 'image' \| 'video' \| 'audio' \| 'document' \| 'sticker', media, … })` | `sendImage`, `sendVideo`, `sendAudio`, `sendDocument`, `sendSticker`, o `sendFile` desde disco |
| `message.download`, `downloadToFile` | `bot.download(message)`, `bot.downloadToFile(message, ruta)` |
| `message.requestMediaReupload` | Automático al descargar algo caducado |
| `message.sendReceipt` | `bot.call('read', …)`, `markPlayed` |
| `message.getReachoutTimelock`, `getNewChatMessageCapping` | `reachoutTimelock`, `newChatMessageCap` |
| `group.createGroup`, `queryGroupMetadata`, `queryAllGroups` | `groupCreate`, `groupMetadata`, `joinedGroups` |
| `group.addParticipants`, `removeParticipants`, `promoteParticipants`, `demoteParticipants` | `groupAdd`, `groupKick`, `groupPromote`, `groupDemote` |
| `group.setSubject`, `setDescription`, `setSetting`, `setEphemeralDuration` | `groupSubject`, `groupDescription`, `groupSetting` |
| `group.queryInviteCode`, `revokeInvite`, `joinGroupViaInvite`, `queryGroupInviteInfo` | `groupInviteCode`, `groupRevokeInvite`, `groupJoin`, `groupInviteInfo` |
| `group.queryMembershipApprovalRequests`, `approve…` / `rejectMembershipRequests` | `groupRequests`, `groupRespondRequests` |
| `group.leaveGroup` | `groupLeave` |
| `profile.getProfilePicture`, `setProfilePicture`, `deleteProfilePicture` | `profilePicture`, `setProfilePicture`, `removeProfilePicture` |
| `profile.getAboutStatus`, `setStatus`, `setPushName` | `fetchAbout`, `setAbout`, `setPushName` |
| `profile.getLidsByPhoneNumbers` | `lidForPhone`, `phoneForLid` |
| `privacy.blockUser`, `unblockUser`, `getBlocklist` | `block`, `unblock`, `blocklist` |
| `privacy.getPrivacySettings`, `setPrivacySetting` | `privacySettings`, `setPrivacy` |
| `presence.send`, `presence.sendChatstate`, `presence.subscribe` | `setOnline`, `typing`; suscribirse a la presencia de otro, solo en Rust con `subscribe_presence` |
| `chat.setChatArchive`, `setChatPin`, `setChatMute`, `setChatRead`, `deleteChat`, `clearChat` | `modifyChat(chat, 'archive' \| 'pin' \| 'mute' \| 'read' \| 'delete' \| 'clear', segundos)`, y sus contrarios `unarchive`, `unpin`… |
| `chat.setMessageStar`, `deleteMessageForMe` | En Rust `star_message` y `chat_modify`; `revoke` borra para todos |
| `status.publishStatusMessage` | `send_status` en Rust |
| `lowlevel` (nodos binarios) | `sendProto`, y en Rust `send_node` y `query` |

Lo que zapo tiene y SoblendWA no: `mobile` (actuar como teléfono principal), `email`, llamadas con `@zapo-js/voip` y la telemetría WAM. Al revés, zapo no trae outbox persistente, límite de envíos con fila por chat, multimedia compartida entre cuentas ni el WhatsApp falso dentro del mismo proceso.

=== pruebas | Pruebas | Operación
lead: Prueba tu bot sin arriesgar ningún número, y vigila que siga funcionando con el tiempo.

## WhatsApp falso {#pruebas-falso}

`FakeWhatsApp` (feature `testing`) es un servidor de WhatsApp en memoria que habla el protocolo real: handshake Noise con su propia cadena de certificados, nodos binarios, reparto de pre-keys, USync, grupos con sender keys, acks, recibos y mensajes guardados para cuentas desconectadas. Las cuentas son clientes SoblendWA normales, así que prueba tu bot completo, cifrado incluido.

```rust
use soblendwa::testing::FakeWhatsApp;
use soblendwa::Event;

let fake = FakeWhatsApp::start().await?;
let (bot, mut eventos) = fake.connect("5215511111111").await?;
let (ana, _) = fake.connect("5215522222222").await?;

ana.send_text(&FakeWhatsApp::jid("5215511111111"), "!ping").await?;
while let Some(evento) = eventos.recv().await {
    if let Event::Message(msg) = evento {
        assert_eq!(msg.text(), Some("!ping"));
        break;
    }
}
```

Simula también la subida y descarga de multimedia, y permite provocar fallos.

| Método | Qué provoca |
|---|---|
| `drop_connections(teléfono)` | Corta la conexión de esa cuenta |
| `refuse_connections(teléfono, true)` | Rechaza sus reconexiones |
| `block(teléfono, 403)` | Bloquea la cuenta con ese código |
| `unblock(teléfono)` | La desbloquea |
| `hold_acks(teléfono, true)` | Retiene las confirmaciones |
| `corrupt_next(teléfono, n)` | Daña los siguientes `n` mensajes |
| `expire_media()` | Caduca los archivos subidos |
| `slow_uploads(duración)` | Hace que cada subida tarde eso |
| `push_node(teléfono, nodo)` | Envía un nodo cualquiera a esa cuenta |
| `create_group(asunto, creador, miembros)` | Crea un grupo |
| `media_uploads()`, `routed()`, `is_online(teléfono)` | Consultan contadores y estado |
| `builder()`, `connect_with(builder)` | Conectan una cuenta con tu propia configuración |

## Desde Node.js {#pruebas-node}

El mismo WhatsApp falso funciona dentro de tu proceso de Node.js, como `@zapo-js/fake-server`. Cada cuenta es un `SoblendWA` normal, así que tus pruebas usan el mismo código que tu bot.

```js
const assert = require('node:assert/strict')
const { FakeWhatsApp } = require('soblendwa-node')

const fake = await FakeWhatsApp.start()
const bot = await fake.connect('5215511111111')
const ana = await fake.connect('5215522222222')

const llega = bot.waitFor('message', (m) => m.text === '!ping')
await ana.sendText('5215511111111', '!ping')
const mensaje = await llega
await bot.reply(mensaje, 'Pong')

const entra = bot.waitFor('joinedGroup')
const grupo = await ana.groupCreate('Pruebas', ['5215511111111'])
assert.equal((await entra).via, 'created')
```

| Método de `FakeWhatsApp` | Qué hace |
|---|---|
| `start()` | Arranca el servidor en un puerto libre |
| `connect(teléfono, opciones)` | Vincula una cuenta y espera a que esté conectada |
| `createGroup(asunto, creador, miembros)` | Crea un grupo y avisa a sus miembros |
| `addToGroup(grupo, por, teléfonos)`, `removeFromGroup(…)` | Añade o saca gente como si lo hiciera un administrador |
| `inviteCode(grupo)`, `setJoinApproval(grupo, true)` | Código de invitación y grupos con aprobación |
| `dropConnections`, `refuseConnections`, `block`, `unblock`, `holdAcks`, `corruptNext`, `expireMedia` | Provocan fallos |
| `slowUploads(ms)` | Hace que cada subida tarde eso, como un CDN lento |
| `mediaUploads()`, `routed()`, `isOnline(teléfono)` | Contadores y estado |

`waitFor(tipo, filtro, espera)` está en todas las cuentas: espera un evento y falla con `TIMEOUT` si no llega. `bindings/node/test-fake.js` es un ejemplo completo con grupos, menciones, vistas previas, errores, reinicios y multimedia compartida.

## Desde Python {#pruebas-python}

Lo mismo con `asyncio`. Los métodos son los de Node.js en snake_case, y `slow_uploads` recibe segundos.

```python
import asyncio
from soblendwa import FakeWhatsApp


async def main():
    fake = await FakeWhatsApp.start()
    bot = await fake.connect("5215511111111")
    ana = await fake.connect("5215522222222")

    await ana.send_text("5215511111111", "!ping")
    mensaje = await bot.wait_for("message", lambda m: m["text"] == "!ping")
    await bot.reply(mensaje, "Pong")

    grupo = await ana.group_create("Pruebas", ["5215511111111"])
    entra = await bot.wait_for("joinedGroup")
    assert entra["via"] == "created" and entra["group"] == grupo["jid"]


asyncio.run(main())
```

`wait_for(tipo, filtro=None, timeout=15)` descarta los eventos que no coinciden y falla con `SoblendError` de código `TIMEOUT`. Un timeout no pierde eventos: el siguiente `wait_for` o `events()` recibe lo que llegó. El ejemplo completo está en `bindings/python/test_fake.py`.

## Desde otros lenguajes

Levanta el servidor contra el WhatsApp falso. Cada sesión recibe un número de prueba, y tu bot se conecta igual que siempre.

```bash Terminal
soblendwa-server --fake --session bot --session ana
```

```js
import { connect } from 'soblendwa-client'

const bot = await connect('ws://127.0.0.1:8787', { session: 'bot' })
const ana = await connect('ws://127.0.0.1:8787', { session: 'ana' })

bot.on('message', (message) => console.log(message.text))
```

Nada sale de tu máquina y las sesiones viven en memoria.

## Contra un servidor falso de otros

SoblendWA también pasa contra `@zapo-js/fake-server`: handshake Noise y certificado, vinculación por QR con firma ADV, mensajes privados y de grupo. Desde Node.js:

```js
const bot = await SoblendWA.create({ server: { wsUrl: servidor.url, noiseRootHex: raiz } })
```

## Las pruebas del proyecto

```bash Terminal
cargo test --features "sqlite mongo testing" --lib
cargo test -p soblendwa-server
cargo test --release -- --ignored throughput
cargo test -p soblendwa-server -- --ignored
```

| Comando | Qué prueba |
|---|---|
| `cargo test --features "sqlite mongo testing" --lib` | El núcleo: más de cien pruebas, con flujos completos contra el WhatsApp falso |
| `cargo test -p soblendwa-server` | HTTP y WebSocket del servidor |
| `cargo test --release -- --ignored throughput` | El rendimiento de [las tablas](#rendimiento) |
| `cargo test -p soblendwa-server -- --ignored` | Se conecta a WhatsApp y espera el QR por WebSocket |
| `node bindings/node/test.js` | El módulo de Node.js |
| `node bindings/node/test-fake.js` | Node.js contra el WhatsApp falso: grupos, errores, límites, subidas, orden |
| `python bindings/python/test_bindings.py` | El módulo de Python |
| `python bindings/python/test_fake.py` | Python contra el WhatsApp falso |
| `node clients/js/test.mjs` | El cliente JS. Usa `SOBLENDWA_URL` y `SOBLENDWA_TOKEN`, con un servidor corriendo |
| `node tests/zapo/round-trip.cjs mongo <url>` | [Migración de ida y vuelta](#migrar-zapo-ida-y-vuelta) con zapo de verdad, también con `sqlite` |
| `node tests/zapo/versus.cjs` | La [comparación con zapo](#rendimiento) con base de datos, red y multimedia |
| `python site/build.py --check` | Que la documentación publicada esté al día, sin enlaces rotos ni versiones viejas |
| `python site/check_snippets.py` | Que cada ejemplo de Rust de la documentación compile |

Incluye pruebas de robustez del decodificador binario: cientos de miles de entradas aleatorias y mutadas, anidamiento hostil y bombas de compresión. Ninguna entrada puede provocar un pánico, un desbordamiento de pila ni agotar la memoria.

El CI de GitHub corre formato, clippy y todas las pruebas en Linux, Windows y macOS, compila con Rust 1.89, levanta el servidor con `--fake` para pasarle el harness desde JavaScript, corre Node.js y Python contra el WhatsApp falso, la migración de zapo contra MongoDB 7, y revisa la documentación en Chrome.

## Bot de pruebas en vivo

`cargo run --release --example bot` dibuja el QR en la terminal y responde unos 30 comandos, uno por función. Escribe `!ayuda` desde otro número. Si un comando falla, el error te llega como respuesta en el mismo chat.

| Comando | Qué hace |
|---|---|
| `!ping` | Responde `Pong` y el tiempo que tardó el bot |
| `!prueba` | Revisa de una vez la latencia, los tiempos por fase, la reutilización de multimedia y el outbox |
| `!stats` | Muestra las métricas |
| `!quien` | Muestra el número y el LID de quien escribe |
| `!outbox` | Muestra lo que sigue pendiente |

La sesión se guarda en `./session`, o en la carpeta que le pases: `--example bot -- mi-carpeta`.

## Pruebas de días

```bash Terminal
node clients/js/soak.mjs --sessions bot,ana --hours 72 --pid 12345
```

Manda texto y multimedia entre cuentas vinculadas durante días, y escribe cada minuto en `soak.jsonl` la memoria por proceso, los mensajes perdidos, los que no se pudieron descifrar, las reconexiones y las métricas de cada cuenta. Con `--webhook` avisa en cuanto aparece un mensaje sin descifrar o un bloqueo. Para medir la memoria por cuenta, corre un servidor por cuenta y pasa `--pid cuenta=pid`.

## Canario diario

```bash Terminal
cargo run --release --example canary -- ./sesion-prueba-a ./sesion-prueba-b
```

Conecta dos cuentas de prueba, manda un texto y una imagen de una a otra, comprueba que se descifran y que la imagen descargada es idéntica, y revisa ping, mensajes sin descifrar y pánicos. Con `SOBLENDWA_ALERT_WEBHOOK` (Discord, Slack o cualquier webhook JSON) avisa si algo falla. Prográmalo una vez al día con cron, un timer de systemd o el Programador de tareas, para enterarte el mismo día si WhatsApp cambió algo.
