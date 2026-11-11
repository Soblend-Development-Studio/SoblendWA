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
| Madurez | Versión 0.1, faltan pruebas largas | Años de uso | Años de uso | Estable desde la 1.0 |

`—` significa que no viene incluido o que no lo encontramos en esa versión. Si algo cambió o nos equivocamos, [abre un issue](https://github.com/Soblend-Development-Studio/SoblendWA/issues) y lo corregimos.

Los números medidos están en [Rendimiento](#rendimiento). Para pasar tu bot, ve a [Migrar](#migrar).

## Cuál elegir

- Elige SoblendWA si te importan la velocidad y la memoria, quieres varias cuentas en un servidor pequeño, o tu bot no está en JavaScript ni en Go.
- Elige Baileys o whatsmeow si necesitas lo más probado en producción, con años de uso y una comunidad grande.
- Elige zapo si necesitas llamadas con audio o actuar como teléfono principal.

=== estado | Estado del proyecto | Proyecto
lead: Versión {{version}}. Lo que se probó de verdad y contra qué, y lo que aún no. Para probar lo que falta, mira [Probar en real](#probar-en-real).

## Probado en vivo con un teléfono real {ok}

Comando por comando con el bot de ejemplo: vinculación por QR, mensajes privados y de grupo, incluidos grupos con LID, `!ping`, editar, eliminar para todos, reacciones, fijar, contactos, vista previa de enlaces, encuestas de una y de varias opciones con votos descifrados, eventos de calendario, álbumes, documentos grandes subidos en streaming, descarga verificada a disco, mensajes interactivos, archivar, store de chats y búsqueda, bots de Meta AI, mensajes temporales, límite de contacto nuevo, canales, grupos e invitaciones, y confirmaciones del servidor.

También con una cuenta real y `!prueba`: los mensajes viejos no se ejecutan, con más de 1.000 al arrancar; número y LID resueltos; admin detectado con solo el LID; tiempos por fase; la imagen repetida sale sin volver a subirse, de 1.420 ms a 4 ms; y el outbox se vacía cuando WhatsApp confirma.

Con un número real y `examples/soak.rs`, en noviembre de 2026: vincular con código; la recuperación de mensajes de grupo que llegan sin su sender key, que el reintento trae en menos de un segundo; los tiempos por fase con red real; descargar y reenviar fotos reutilizando lo ya subido; y el aviso `lidMapping`.

## En producción con un número real {#estado-produccion}

Desde el 9 de noviembre de 2026 una cuenta personal real corre `examples/soak.rs` en un VPS Linux de 2 núcleos, vinculada como un dispositivo más, sin responder a nadie más que al dueño. No es un bot de prueba: recibe todo lo que le llega a esa persona, grupos incluidos.

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

De las 25 reconexiones, 7 fueron cortes de red y 1 un `503` de WhatsApp. Las otras 17 eran un `stream:error` que llegaba exactamente 50 minutos después de cada conexión. Para encontrarlo, el motivo de la desconexión ahora trae el nodo completo, y en la siguiente corrida dijo `<ack class="status" type="reaction" …/>`: una reacción a un estado de la cuenta, que llega como `<status>` y que SoblendWA no confirmaba. WhatsApp la volvía a mandar en cada conexión y cortaba a los 50 minutos de no recibir el `ack`. Ya se confirma.

El 10 de noviembre la misma sesión pasó a `bindings/node/soak.js`, la misma prueba hecha con el módulo de Node.js, sin volver a vincular. Así esa prueba cubre también Node.js en Linux con un teléfono real. Los números de arriba son del primer día; la prueba sigue y cuenta como estable cuando sume dos semanas.

## Probado contra los servidores de WhatsApp, sin teléfono {ok}

Conexión a través de un proxy SOCKS5 con usuario y contraseña, los módulos de Node.js y Python, y el servidor WebSocket con el cliente JavaScript, con el QR recibido por WebSocket y la API HTTP.

## Probado contra otras librerías {ok}

- libsignal-node: una sesión creada por libsignal se importa, SoblendWA descifra mensajes pendientes y libsignal descifra las respuestas.
- zapo-js 1.11: importación de su base SQLite y rollback, en los dos sentidos.
- `@zapo-js/fake-server`: handshake Noise y certificado, vinculación por QR con firma ADV, mensajes privados y de grupo, y un store escrito en JavaScript que sobrevive a un reinicio.

## Probado contra el WhatsApp falso {ok}

Mensajes privados y de grupo entre cuentas, mensajes dañados o sin sender key que se recuperan con el reintento, reintentos después de reiniciar, reconexión con reenvío del outbox sin duplicar, cuenta bloqueada que se detiene, límite de envíos, multimedia subida y descargada, también en streaming, vista previa completa con bloqueo de hosts privados, eventos tipados, y el servidor con el cliente JavaScript y el harness de pruebas largas.

## Probado en CI

Las pruebas pasan en Linux, Windows y macOS, y el código compila con Rust 1.89. Se compilan 16 binarios para las distintas plataformas, y cada módulo de Node.js se carga en la suya. Los paquetes publicados se instalaron y ejecutaron en Windows; los de Linux y macOS no se han usado fuera del CI.

## Experimental {exp}

Implementado y probado solo localmente, todavía no contra una cuenta real:

- Crear comunidades y canales.
- Catálogo de empresa y mensajes de producto.
- Re-subida de multimedia caducada.
- Etiquetas, respuestas rápidas y etiquetas de miembro.
- El reenvío del outbox tras una caída real.
- Qué tanto se recuperan con el reintento los mensajes con MAC mala.
- El evento de cuenta bloqueada con un bloqueo real.
- La reutilización de multimedia pasada la hora, con verificación en el CDN.
- La miniatura nítida de la vista previa.
- Los eventos de foto, información, bloqueos y privacidad.
- El canario diario.
- Lo nuevo de la {{minor}} con WhatsApp real: `joinedGroup` y `leftGroup`, la causa de `loggedOut`, `messageLost`, la multimedia compartida entre cuentas y el orden de los envíos.

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

Si prefieres Node.js, `node soak.js <carpeta>` (viene en `soblendwa-node`) hace exactamente lo mismo con la misma carpeta y el mismo reporte, así que puedes pasar de uno a otro sin volver a vincular.

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
lead: Esta documentación es de la versión {{version}}. Abajo está el historial completo, el mismo de `CHANGELOG.md`.

## Qué versión usar

Fija la versión exacta. En 0.x el número del medio puede romper cosas, así que una actualización de `0.1` a `0.2` merece leer la sección **Rompe** de abajo antes de cambiar.

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
