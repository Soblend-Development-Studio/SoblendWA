=== arquitectura | Cómo funciona | Proyecto
lead: Qué ocurre por dentro desde que abres la conexión hasta que sale un mensaje.

## Conectar

:::steps
- WebSocket. Se abre un WSS hacia WhatsApp, a través del proxy si lo configuraste, con `TCP_NODELAY`.
- Handshake Noise. Se negocia `XX_25519_AESGCM_SHA256` y se verifica la cadena de certificados de WhatsApp y su vigencia. A partir de aquí todo va cifrado.
- Inicio de sesión. Si no hay cuenta vinculada, se registra y llegan los QR. Si la hay, se presenta con sus claves.
- Preparación. Se suben pre-keys si hacen falta, se sincroniza el app state, y se precargan tus grupos y los dispositivos de sus miembros.
- Listo. Llega `Event::Connected`, y empiezan el keepalive y el vaciado del outbox.
:::

## Recibir un mensaje

:::steps
- Llega un frame cifrado por el WebSocket y se descifra con AES-GCM.
- El decodificador lo convierte en un nodo binario, con límites contra entradas hostiles.
- Pasa a la cola de su chat, así los chats distintos van en paralelo y cada uno conserva su orden.
- Se descifra con Signal: Double Ratchet en privado, Sender Keys en grupos. Si falla, se pide un reintento.
- Se descarta si es un duplicado, se resuelven el número y el LID, y se desenvuelve el protobuf.
- Se envía el recibo a WhatsApp, y `Event::Message` llega a tu bot.
:::

## Enviar un mensaje

:::steps
- Límite. Se comprueba el [límite de envíos](#confiabilidad-limite), del total y del chat.
- Outbox. El mensaje se guarda, por si se cae la conexión.
- Cola. Entra en la cola de su chat, para conservar el orden.
- Vista previa. Si trae un enlace, se genera. Nunca frena el envío más de lo configurado.
- Dispositivos. Se buscan los del destinatario, en caché, y se crean las sesiones Signal que falten. Es la única parte con esperas de red, y solo la primera vez.
- Cifrado. Se cifra para cada dispositivo. En grupos, se usa la sender key.
- Confirmación. WhatsApp responde, llega `Event::ServerAck` con sus tiempos, y el mensaje sale del outbox.
:::

## Cómo está organizado el código

```text
src/
├── binary/          códec de nodos binarios, diccionario de tokens y pruebas de robustez
├── crypto/          X25519, firmas XEdDSA, AES-GCM/CBC/CTR, HKDF, PBKDF2
├── noise.rs         handshake Noise XX, verificación de certificados y framing
├── proto/           mensajes protobuf de WhatsApp y de Signal
├── signal/          sesiones (X3DH + Double Ratchet) y Sender Keys
├── store/           credenciales, FileStore, MemoryStore, BatchedStore,
│                    SQLite, PostgreSQL, Redis, MongoDB, Baileys y zapo
├── chats.rs         store de chats, contactos y mensajes
├── interactive.rs   botones, listas, plantillas y mensajes interactivos
├── json.rs          eventos, mensajes normalizados, métricas y nodos a JSON
├── testing.rs       WhatsApp falso en memoria con el protocolo real
└── client/          conexión, vinculación, envío, recepción, app state, grupos,
                     comunidades, canales, catálogo, cuenta, multimedia, proxy,
                     presencia, tokens, USync, outbox, vista previa, límite de
                     envíos, colas por chat y métricas
bindings/
├── bridge/          sesión compartida y despachador de métodos por nombre
├── node/            módulo nativo para Node.js (napi-rs)
├── python/          módulo nativo para Python (PyO3, asyncio)
└── server/          servidor WebSocket + HTTP + webhooks (axum)
clients/
└── js/              cliente JavaScript/TypeScript del servidor
examples/            bot de pruebas, bot de eco, vinculación con código, canario
                     e importación de Baileys y de zapo
tests/fixtures/      sesiones con claves y números de prueba, para las migraciones
docs/                este sitio
```

## Seguridad del código

- `#![forbid(unsafe_code)]` en el núcleo.
- El decodificador binario resiste entradas aleatorias, anidamiento hostil y bombas de compresión, sin pánicos ni desbordamientos.
- La multimedia descargada se verifica con HMAC y SHA-256 antes de entregarse.
- El protocolo Signal está probado contra `libsignal-node` en las dos direcciones.

=== comparacion | Comparación | Proyecto
lead: Cómo se compara con otras librerías, incluido lo que ellas tienen y SoblendWA no.

| | SoblendWA | Baileys 7 | whatsmeow | zapo-js 1.11 |
|---|---|---|---|---|
| Lenguaje | Rust | JavaScript | Go | TypeScript |
| Se usa desde | Rust, Node.js, Python y cualquier lenguaje por WebSocket, HTTP o webhooks | JavaScript, TypeScript | Go | JavaScript, TypeScript |
| Almacenamiento incluido | Archivos, memoria, SQLite, PostgreSQL, Redis y MongoDB | Archivos | SQLite y PostgreSQL | SQLite, PostgreSQL, MySQL, Redis y MongoDB, en paquetes aparte |
| Vista previa automática de enlaces | Sí, con bloqueo de hosts privados | Con `link-preview-js` | A mano | Sí |
| Outbox persistente y límite de envíos | Sí | — | — | — |
| Importar la sesión de otra librería | Baileys y zapo, con rollback a zapo | — | — | — |
| WhatsApp falso para pruebas | Incluido | — | — | `@zapo-js/fake-server` |
| Llamadas con audio | — | — | — | `@zapo-js/voip` |
| Actuar como teléfono principal | — | — | — | Sí |
| Madurez | Versión {{version_label}}, con una [prueba larga en producción](#estado-produccion) en marcha | Años de uso | Años de uso | Estable desde la 1.0 |

`—` significa que no viene incluido o que no lo encontramos en esa versión. Si algo cambió o nos equivocamos, [abre un issue](https://github.com/Soblend-Development-Studio/SoblendWA/issues) y lo corregimos.

Los números medidos están en [Rendimiento](#rendimiento). Para pasar tu bot, ve a [Migrar](#migrar).

## Cuál elegir

- Elige SoblendWA si te importan la velocidad y la memoria, quieres varias cuentas en un servidor pequeño, o tu bot no está en JavaScript ni en Go.
- Elige Baileys o whatsmeow si necesitas lo más probado en producción, con años de uso y una comunidad grande.
- Elige zapo si necesitas llamadas con audio o actuar como teléfono principal.

=== estado | Estado del proyecto | Proyecto
lead: Versión {{version_label}}. Lo que se probó de verdad y contra qué, y lo que aún no. Para probar lo que falta, mira [Probar en real](#probar-en-real).

## Probado en vivo con un teléfono real {ok}

Comando por comando con el bot de ejemplo: vinculación por QR, mensajes privados y de grupo, incluidos grupos con LID, `!ping`, editar, eliminar para todos, reacciones, fijar, contactos, vista previa de enlaces, encuestas de una y de varias opciones con votos descifrados, eventos de calendario, álbumes, documentos grandes subidos en streaming, descarga verificada a disco, mensajes interactivos, archivar, store de chats y búsqueda, bots de Meta AI, mensajes temporales, límite de contacto nuevo, canales, grupos e invitaciones, y confirmaciones del servidor.

También con una cuenta real y `!prueba`: los mensajes viejos no se ejecutan, con más de 1.000 al arrancar; número y LID resueltos; admin detectado con solo el LID; tiempos por fase; la imagen repetida sale sin volver a subirse, de 1.420 ms a 4 ms; y el outbox se vacía cuando WhatsApp confirma.

Con dos cuentas reales y un testigo que cuenta lo que llega, 5 a 10 veces cada cosa: el outbox después de un `kill -9`, la recuperación de una sesión dañada y la multimedia compartida entre cuentas. Los resultados están en [Pruebas repetidas](#estado-pruebas-repetidas).

Con un número real y `examples/soak.rs`, en noviembre de 2026: vincular con código; la recuperación de mensajes de grupo que llegan sin su sender key, que el reintento trae en menos de un segundo; los tiempos por fase con red real; descargar y reenviar fotos reutilizando lo ya subido; y el aviso `lidMapping`.

## En producción con un número real {run} {#estado-produccion}

Una cuenta personal real corre la prueba larga en un VPS Linux de 2 núcleos, vinculada como un dispositivo más y sin responder a nadie más que al dueño. No es un bot de prueba: recibe todo lo que le llega a esa persona, grupos incluidos. Empezó el 9 de noviembre de 2026 a las 22:53, hora de Colombia, con `examples/soak.rs`. El 10 de noviembre a las 19:06 la misma sesión pasó a `bindings/node/soak.js`, sin volver a vincular. Cuenta como estable cuando sume dos semanas.

| Primeras 19,5 horas, sin reiniciar el proceso | |
|---|---|
| Mensajes recibidos | 27.963 |
| Sin descifrar al llegar | 79 (0,28 %), todos por sender key de grupo |
| Recuperados con el reintento | 66, uno de ellos después de darlo por perdido |
| Perdidos tras 10 minutos de espera | 10 (0,04 % de lo recibido) |
| Todavía esperando al momento del corte | 3 |
| Pánicos | 0 |
| Memoria máxima | 27 MB |
| Reconexiones | 25, cada una en 1,0 a 1,9 s |

De las 25 reconexiones, 7 fueron cortes de red y 1 un `503` de WhatsApp. Las otras 17 eran un `stream:error` que llegaba exactamente 50 minutos después de cada conexión. Para encontrarlo, el motivo de la desconexión ahora trae el nodo completo, y en la siguiente corrida dijo `<ack class="status" type="reaction" …/>`: una reacción a un estado de la cuenta, que llega como `<status>` y que SoblendWA no confirmaba. WhatsApp la volvía a mandar en cada conexión y cortaba a los 50 minutos de no recibir el `ack`. Ya se confirma: con el arreglo desplegado, la misma cuenta pasó 16 horas seguidas sin un solo `stream:error`; los 6 cortes de ese tiempo fueron caídas de red, y cada uno se recuperó solo.

Los días se cuentan de reloj desde el primer arranque, con los reinicios dentro. El 11 de noviembre a las 13:20 iban 1,6 días (38 horas) y 4 arranques: 46.260 mensajes recibidos, 110 sin descifrar al llegar (0,24 %, todos por sender key), 91 recuperados, 15 perdidos y 0 pánicos.

Día por día, con `node soak.js <carpeta> --por-dia`, hasta el 11 de noviembre a las 19:45:

| Día | Proceso | Recibidos | Sin descifrar | Recuperados | Perdidos | Cortes | Memoria máx. | Arranques |
|---|---|---|---|---|---|---|---|---|
| 9 de noviembre, desde las 22:53 | Rust | 3.273 | 8 | 8 | 0 | 1 `stream:error` | 21 MB | 1 |
| 10 de noviembre, hasta las 19:06 | Rust | 26.811 | 72 | 59 | 11 | 17 `stream:error`, 7 de red, 1 `503` | 27 MB | 0 |
| 10 de noviembre, desde las 19:06 | Node.js | 14.507 | 13 | 9 | 3 | 1 `stream:error`, 1 de red | 123 MB | 3 |
| 11 de noviembre | Node.js | 4.428 | 65 | 55 | 8 | 7 de red, 2 `503`, 1 de prueba, 1 reinicio | 139 MB | 20 |

Los `stream:error` del 9 y el 10 de noviembre son el bug de las reacciones a estados, ya arreglado. El 11 de noviembre incluye las [pruebas repetidas](#estado-pruebas-repetidas): 15 `kill -9`, 12 sesiones dañadas a propósito y los reinicios para desplegar. Los recibidos salen de una foto de los contadores cada 10 minutos, así que un `kill -9` pierde lo que pasó desde la última.

La memoria de Rust y la de Node.js de esa corrida no se pueden comparar: los 27 MB de Rust son de las primeras 19,5 horas, y los 134 MB de Node.js, con V8 incluido, son del 10 de noviembre en adelante, con otro tráfico. Desde el 11 de noviembre corre también un proceso en Rust, el testigo de abajo, pero en otra cuenta con mucho menos tráfico, así que tampoco sirve para compararlos. Comparar con el mismo tráfico pide los dos procesos en la misma cuenta a la vez, y eso sigue pendiente.

### Pruebas repetidas con dos cuentas {#estado-pruebas-repetidas}

El 11 de noviembre de 2026, de 17:38 a 17:50, con la cuenta de la prueba larga y una segunda cuenta real como [testigo](#probar-testigo): otro número, vinculado con `examples/soak.rs`, que anota lo que de verdad le llega. Las tres series corrieron solas en el VPS, sin tocar ningún teléfono.

| Prueba | Rondas | Resultado |
|---|---|---|
| 5 mensajes y `kill -9` a mitad del envío, entre 25 y 354 ms después | 10 | 30 aceptados y 30 recibidos por el testigo, 0 duplicados, en orden las 10 veces; la cola reenvió 17 al volver |
| Imagen nueva, `kill -9`, y la misma imagen otra vez desde el mismo proceso y desde la otra cuenta | 5 | Reutilizada 5 de 5 después del `kill -9`, y 5 de 5 desde la otra cuenta, por el store compartido |
| Sesión con la otra cuenta dañada, y su respuesta de vuelta por esa sesión | 10 | 10 de 10 recuperadas con un reintento, entre 925 y 934 ms; ninguna perdida |

En 4 de las 10 rondas de `kill -9` el proceso murió antes de aceptar ningún envío, mientras pedía los dispositivos del contacto nuevo, y ninguno de esos mensajes llegó. Es lo correcto: nadie había recibido un id. Preparando esta serie apareció un bug real: `send_*` devolvía el id antes de que el outbox estuviera escrito, porque los stores escriben hasta 50 ms después, así que un `kill -9` en ese hueco perdía un mensaje ya aceptado. Una prueba lo reproduce, y estaba arreglado antes de correr la serie.

El testigo es una cuenta recién vinculada, y ahí apareció algo que la cuenta de la prueba larga no muestra. En sus primeras dos horas, 83 mensajes que le llegaron no se pudieron descifrar por MAC mala, y 73 no volvieron con el reintento. En la cuenta larga son 6 en dos días, sin contar las pruebas. Está en investigación. También se vio que un mensaje de grupo ya descifrado que WhatsApp vuelve a entregar después de un reinicio se cuenta como fallo, cuando es un duplicado.

## Probado contra los servidores de WhatsApp, sin teléfono {ok}

Conexión a través de un proxy SOCKS5 con usuario y contraseña, los módulos de Node.js y Python, y el servidor WebSocket con el cliente JavaScript, con el QR recibido por WebSocket y la API HTTP.

## Probado contra otras librerías {ok}

- libsignal-node: una sesión creada por libsignal se importa, SoblendWA descifra mensajes pendientes y libsignal descifra las respuestas.
- zapo-js 1.11: importación de su base SQLite y rollback, en los dos sentidos.
- `@zapo-js/fake-server`: handshake Noise y certificado, vinculación por QR con firma ADV, mensajes privados y de grupo, y un store escrito en JavaScript que sobrevive a un reinicio.

## Probado contra el WhatsApp falso {ok}

Mensajes privados y de grupo entre cuentas, mensajes dañados o sin sender key que se recuperan con el reintento, reintentos después de reiniciar, reconexión con reenvío del outbox sin duplicar, cuenta bloqueada que se detiene, límite de envíos, multimedia subida y descargada, también en streaming, vista previa completa con bloqueo de hosts privados, eventos tipados, y el servidor con el cliente JavaScript y el harness de pruebas largas.

## Probado en CI

Las pruebas pasan en Linux, Windows y macOS, y el código compila con Rust 1.89. Se compilan 16 binarios para las distintas plataformas, cada módulo de Node.js se carga en la suya, y los módulos de Node.js y Python corren sus pruebas contra el WhatsApp falso. Después de cada publicación, `published.yml` instala lo que quedó en npm, PyPI y crates.io en máquinas limpias de Linux (x64 y arm64), macOS y Windows, y conecta con WhatsApp hasta recibir el QR, sin vincular nada. Con la 0.2.0, el 11 de noviembre de 2026, pasó en las 8. Fuera de esas máquinas, en macOS los paquetes todavía no los ha usado nadie. Lo que corre con un número real está en [En producción](#estado-produccion), no aquí.

## Experimental {exp}

Implementado y probado solo localmente, todavía no contra una cuenta real:

- Crear comunidades y canales.
- Catálogo de empresa y mensajes de producto.
- Re-subida de multimedia caducada.
- Etiquetas, respuestas rápidas y etiquetas de miembro.
- El evento de cuenta bloqueada con un bloqueo real.
- La reutilización de multimedia pasada la hora, con verificación en el CDN.
- La miniatura nítida de la vista previa.
- Los eventos de foto, información, bloqueos y privacidad.
- El canario diario.
- Lo nuevo de la {{minor}} con WhatsApp real: `joinedGroup` y `leftGroup`, y la causa de `loggedOut`.

`PostgresStore` y `RedisStore` ya pasan contra servidores reales en el CI (PostgreSQL 16 y Redis 7), pero todavía no se han usado con datos de producción. `MongoStore` y la importación desde `@zapo-js/store-mongo` dejaron de ser experimentales: el CI migra una sesión que escribió zapo en MongoDB 7, sigue las mismas conversaciones y la devuelve ([cómo](#migrar-zapo-ida-y-vuelta)).

Si alguno falla con tu cuenta, [abre un issue](https://github.com/Soblend-Development-Studio/SoblendWA/issues) con el error.

## No incluido {no}

A diferencia de zapo, SoblendWA no incluye:

- Actuar como teléfono principal: registro móvil y vincular otros dispositivos.
- Mensajes de Meta AI con su cifrado propio (`msmsg`).
- Confirmación por passkey al vincular.
- Llamadas con audio y video. Solo recibe los eventos, las rechaza y crea enlaces.
- Telemetría WAM.
- Un store de MySQL incluido, aunque puedes [escribirlo tú](#almacenamiento-store-escrito-en-rust).

=== probar-en-real | Probar en real | Proyecto
lead: Cómo quitarle lo experimental a cada parte. Lo que se puede automatizar ya corre en el CI; lo demás necesita un número de prueba y tiempo.

## Qué cuenta como probado

Una parte deja de ser experimental cuando pasa contra lo real (un servidor de base de datos de verdad, WhatsApp con un número de prueba) y no falla en el uso normal. Para llamar estable a una versión, además: dos semanas seguidas en producción sin caídas ni bloqueos, y con tantos o menos mensajes sin descifrar que con la librería anterior.

Usa siempre un número de prueba y una base de prueba. Nada de esto necesita tu número principal ni tus datos reales.

## 1. Bases de datos {#probar-bases}

`store-check.js` vincula dos cuentas del WhatsApp falso guardando todo en tu base, manda privados en orden, mensajes de grupo y una imagen, reinicia las dos cuentas leyendo todo desde la base y vuelve a comprobar que nada se perdió. No se conecta a WhatsApp real. Corre donde tengas Node.js y el módulo instalado:

```bash Terminal
node node_modules/soblendwa-node/store-check.js '{"type":"mongo","url":"mongodb://localhost:27017","database":"soblendwa_prueba"}'
node node_modules/soblendwa-node/store-check.js '{"type":"postgres","url":"postgres://usuario:clave@localhost/soblendwa_prueba"}'
node node_modules/soblendwa-node/store-check.js '{"type":"redis","url":"redis://127.0.0.1/"}'
```

Cada paso imprime `ok` y cuánto tardó. Al final las claves de prueba se borran y quedan solo dos filas de credenciales sin vincular. Desde el repositorio también puedes correr las pruebas de Rust contra el mismo servidor:

```bash Terminal
SOBLEND_MONGO_URL=mongodb://localhost:27017 cargo test --features "stores testing" --lib -- --ignored mongo_roundtrip zapo_mongo_live
SOBLEND_POSTGRES_URL=postgres://usuario:clave@localhost/soblendwa_prueba cargo test --features "stores testing" --lib -- --ignored postgres_roundtrip
SOBLEND_REDIS_URL=redis://127.0.0.1/ cargo test --features "stores testing" --lib -- --ignored redis_roundtrip
```

El trabajo `real-databases` del CI hace todo esto en cada push, con MongoDB 7, PostgreSQL 16 y Redis 7.

## 2. Migrar desde zapo con MongoDB {#probar-migracion}

Sigue [la copia de tu base](#migrar-copia) con un subbot, nunca con el principal. En corto:

```js Node.js
const { importZapo, rollbackZapo } = require('soblendwa-node')

const copia = { type: 'mongo', url: 'mongodb://localhost:27017', database: 'bot_copia', session: 'subbot1' }
const destino = { type: 'mongo', url: 'mongodb://localhost:27017', database: 'soblendwa_prueba', account: 'subbot1' }

console.log(await importZapo(copia, null, { check: true }))
console.log(await importZapo(copia, destino))
```

Con el subbot de zapo apagado, arranca `live-check.js` (abajo) con `--store` apuntando a `destino`. Si conecta sin pedir QR y responde, la migración funcionó. Para volver, apágalo y corre `rollbackZapo(destino, copia)`, arranca zapo con `bot_copia` y comprueba que sigue respondiendo sin volver a vincular.

## 3. WhatsApp real con un número de prueba {#probar-en-vivo}

`live-check.js` es un bot que responde comandos para probar cada función nueva y guarda en un reporte todo lo que pasa: entradas y salidas de grupos, mensajes sin descifrar, recuperados y perdidos, desconexiones, la causa de cada cierre de sesión y, cada 10 minutos, las métricas y la memoria.

```bash Terminal
node node_modules/soblendwa-node/live-check.js --pair 5215512345678
```

La sesión queda en `prueba-en-vivo/auth.db` y el reporte en `prueba-en-vivo/live-check.jsonl`. Desde otro teléfono:

| Haz esto | Comprueba |
|---|---|
| `!ping`, `!estado`, `!fases` | Latencia, métricas y tiempos por fase con tu red real |
| `!lid` | Número y LID, y la consulta en los dos sentidos |
| `!pfp` con tu foto privada | Que avise `PRIVATE_PHOTO` |
| `!cita` | Respuesta con cita y mención |
| Manda una imagen y luego `!foto` dos veces | Descarga, reenvío y que la segunda vez no se vuelva a subir |
| `!rafaga` | Cinco mensajes seguidos que llegan en orden |
| Crea un grupo con el bot dentro, agrégalo a otro y mándale un enlace | `joinedGroup` con `created`, `added` e `invite`, y el saludo con miniatura propia |
| `!unir` con el enlace de un grupo con aprobación | `APPROVAL_REQUIRED` |
| Haz admin al bot y usa `!nombre`, `!desc`, `!cerrar`, `!abrir`, `!admin`, `!sacar` | Las acciones de grupo |
| Saca al bot de un grupo | `leftGroup` con `removed: true` |
| Al final, quita el dispositivo desde Dispositivos vinculados | `loggedOut` con `cause: removedFromPhone` |

Si algo falla, el bot responde con el código del error y lo anota en el reporte.

## 4. Dos semanas {#probar-semanas}

Deja corriendo `live-check.js`, o tu bot con un subbot, durante dos semanas en el mismo servidor donde corre zapo. Si se reinicia, el reporte sigue sumando. Cuando quieras ver cómo va:

```bash Terminal
node node_modules/soblendwa-node/live-check.js --resumen prueba-en-vivo/live-check.jsonl
```

El resumen da los días, los arranques, los mensajes recibidos y enviados, los que no se pudieron descifrar (en porcentaje y por día, para compararlos con zapo), los recuperados, los perdidos de verdad, las reconexiones, la memoria máxima, los bloqueos y cada cierre de sesión con su causa.

Además, el [canario](#pruebas) entre dos números de prueba, una vez al día, avisa el mismo día si WhatsApp cambió algo.

## Si solo tienes un número {#probar-un-numero}

`live-check.js` responde a mensajes de otros, así que necesita dos números. Con uno solo, usa `examples/soak.rs`: se vincula a tu propio WhatsApp como un dispositivo más, no te pone en línea, no marca nada como leído y no le contesta a nadie. Solo te responde a ti cuando escribes `!yowner` en un chat privado, mejor en el chat contigo mismo.

```bash Terminal
cargo run --release --example soak -- --pair 5215512345678
```

| Escribe | Qué hace |
|---|---|
| `!yowner` | Resumen de la prueba: recibidos, sin descifrar por tipo, recuperados, perdidos, reconexiones y memoria |
| `!yowner ping` | Cuánto tarda el bot y la red |
| `!yowner fases` | Tiempos de los envíos por fase, con tu red |
| `!yowner cita` | Te responde citándote y mencionándote |
| `!yowner lid` | Tu número, tu LID y la consulta en los dos sentidos |
| `!yowner foto` | Reenvía la última foto que mandaste en ese chat; la segunda vez no se vuelve a subir |

Si quieres ver la prueba día por día, `node soak.js <carpeta> --por-dia` arma una tabla con los recibidos, los que no se pudieron descifrar, los recuperados, los perdidos, los cortes con su causa, la memoria máxima y los arranques, separando lo que corrió en Rust de lo que corrió en Node.js. Sirve con la carpeta de cualquiera de los dos.

Si prefieres Node.js, `node soak.js <carpeta>` (viene en `soblendwa-node`) hace exactamente lo mismo con la misma carpeta y el mismo reporte, así que puedes pasar de uno a otro sin volver a vincular.

La versión de Node.js trae además comandos para provocar, con tu propio número, los casos que en la vida real dependen de la mala suerte:

| Comando | Qué prueba | Qué tienes que ver |
|---|---|---|
| `!yowner mac` | Daña solo la sesión del bot con tu teléfono, como un disco corrupto | Manda un mensaje cualquiera desde el teléfono. En `!yowner` debe salir `bad_mac` recuperado |
| `!yowner cola` | Te manda 3 mensajes y corta la conexión antes de que WhatsApp los confirme | `cola 1/3`, `2/3` y `3/3` una sola vez cada uno y en orden |
| `!yowner media` y `!yowner reiniciar` | La multimedia guardada sobrevive a reiniciar el proceso | La segunda vez, después de reiniciar, dice que no se volvió a subir |

`!yowner mac` no toca tu teléfono ni tus chats con otros: solo el estado del bot. Usan `simulate_outage` y `damage_sessions`, que existen con la feature `testing` para esto mismo. El aviso de cuenta bloqueada no tiene comando: provocarlo de verdad es arriesgar la cuenta.

### Repetirlas con un testigo {#probar-testigo}

Una prueba que sale bien una vez no dice mucho. Para repetirlas y contar lo que llegó de verdad hace falta un testigo: `examples/soak.rs` vinculado aparte, con otra palabra para sus comandos, que anota en su reporte las pruebas que le llegan y no contesta tus `!yowner`.

Lo mejor es ponerlo en un segundo número. `soak.js` le manda las pruebas como a cualquier contacto, el testigo solo anota las que vienen de tu número (`--testigo-de`), la multimedia compartida se prueba entre dos cuentas y `!yowner mac 10` corre sin tu teléfono: daña la sesión con el testigo y le pide una respuesta, que vuelve por esa sesión dañada.

```bash Terminal
cargo run --release --example soak --features sqlite -- sesion-testigo --comando '!yrust' --compartida compartida --testigo-de 5215512345678 --pair 5215598765432
node soak.js sesion-node --testigo sesion-testigo/soak.jsonl --numero-testigo 5215598765432 --compartida compartida
```

Con un solo número, el testigo puede ser un segundo dispositivo del mismo: quita `--testigo-de` y `--numero-testigo`, y vincúlalo con tu propio número. Entonces `!yowner mac` necesita tu teléfono, como arriba.

| Comando | Qué hace cada ronda | Qué cuenta |
|---|---|---|
| `!yowner mac 10` | Con el testigo en otro número: daña la sesión con él y espera su respuesta. Sin él, cada `!yowner mac` que mandas llega dañado y mide la ronda anterior | Cuánto tardó en volver, o si se perdió |
| `!yowner caida 10` | Manda 5 mensajes y mata el proceso con `kill -9` a un tiempo al azar entre 0 y 400 ms. systemd lo levanta, y a los 20 s compara con el testigo | Aceptados que no llegaron, duplicados y orden |
| `!yowner compartida 5` | Sube una imagen nueva y mata el proceso con `kill -9`; al volver la manda otra vez, y el testigo la manda desde el store de multimedia compartido | Si se reutilizó después del `kill -9` y desde el otro dispositivo |
| `!yowner pruebas` | | La tabla de todas las rondas |
| `!yowner parar` | | No empieza más rondas |

Después de un `kill -9`, WhatsApp puede volver a entregar el `!yowner` que lo provocó, así que `soak.js` anota los comandos que ya corrió y no repite ninguno. `node soak.js sesion-node --pruebas` saca la misma tabla sin conectarse, ronda por ronda.

En un servidor también se lanzan sin escribir en WhatsApp: pon lo que irías a escribir después de `!yowner` en `<carpeta>/orden` y mándale `SIGUSR2` al proceso. Las respuestas llegan a tu chat igual que si lo hubieras escrito.

```bash Terminal
echo "caida 10" > sesion-node/orden && kill -USR2 "$(pgrep -f 'soak.js sesion-node')"
```

Con la cuenta de la prueba larga, el 11 de noviembre de 2026:

- `cola`: después del corte, los 3 mensajes salieron por la cola.
- `media` y `reiniciar`: después de reiniciar el proceso, la imagen salió de la multimedia guardada, dos veces seguidas.
- `mac`: el mensaje se recuperó en 1,5 s, pero con dos reintentos. El primero no llevaba llaves y el teléfono reenvió por la misma sesión dañada, así que falló otra vez. Con el primer reintento llevando las llaves, la misma prueba dio un solo `bad_mac`, recuperado en 0,8 s.

El reporte va en `<carpeta>/soak.jsonl` y solo guarda conteos y tipos de error, sin textos ni números. `cargo run --example soak -- --resumen` lo resume sin conectarse. Para dejarlo semanas en un servidor, córrelo como servicio de systemd con `Restart=on-failure`: si quitas el dispositivo desde el teléfono, termina sin error y no se vuelve a levantar solo.

## Cuando termines

Abre un [issue](https://github.com/Soblend-Development-Studio/SoblendWA/issues) con la salida de `--resumen` y los errores que hayas visto. Con eso se mueve cada parte de experimental a probado en esta página y en el README.

=== aviso | Aviso y licencia | Proyecto
lead: Lo que debes saber antes de usar SoblendWA con una cuenta real.

## Aviso

SoblendWA es un proyecto independiente. No está afiliado, asociado ni respaldado por WhatsApp ni por Meta.

Automatizar una cuenta puede ir contra los Términos de Servicio de WhatsApp y provocar que la bloqueen. Úsalo con responsabilidad y no envíes spam. Ningún límite ni ajuste de la librería garantiza que una cuenta no sea bloqueada.

## Licencia

MIT © 2026 **Soblend Development Studio**. El texto completo está en [LICENSE](https://github.com/Soblend-Development-Studio/SoblendWA/blob/main/LICENSE).

## Agradecimientos

SoblendWA es una implementación propia, y existe gracias a lo que documentaron y publicaron [Baileys](https://github.com/WhiskeySockets/Baileys), [whatsmeow](https://github.com/tulir/whatsmeow), libsignal y zapo.

## Dónde conseguir ayuda

- Errores y propuestas: [issues en GitHub](https://github.com/Soblend-Development-Studio/SoblendWA/issues).
- Código: [GitHub](https://github.com/Soblend-Development-Studio/SoblendWA).
- Paquetes: [crates.io](https://crates.io/crates/soblendwa), [npm](https://www.npmjs.com/package/soblendwa-node), [cliente en npm](https://www.npmjs.com/package/soblendwa-client) y [PyPI](https://pypi.org/project/soblendwa/).
- Documentación generada de Rust: [docs.rs](https://docs.rs/soblendwa).

=== versiones | Versiones y cambios | Proyecto
lead: Esta documentación es de la versión {{version_label}}. Abajo está el historial completo, el mismo de `CHANGELOG.md`.

{{release_note}}

## Qué versión usar

Fija la versión exacta que instalas, sin `^` ni `~`. En 0.x el número del medio puede romper cosas, así que antes de pasar de una versión `0.x` a la siguiente lee su sección **Rompe** abajo. La última publicada es la {{published}}.

:::tabs
```toml Rust
soblendwa = "={{version}}"
```
```json Node.js
{ "dependencies": { "soblendwa-node": "{{version}}" } }
```
```text Python
soblendwa=={{version}}
```
:::

Cada versión publicada tiene su etiqueta `v…` en GitHub con los binarios del servidor y los módulos, y los paquetes de crates.io, npm y PyPI se publican desde esa misma etiqueta.

@include CHANGELOG.md
