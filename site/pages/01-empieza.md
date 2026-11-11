=== inicio | Inicio | Empieza | bare

:::raw
<header class="home-hero">
  <p class="eyebrow">Docs de SoblendWA {{minor}}</p>
  <h1>Bots de WhatsApp con un motor en Rust.</h1>
  <p class="lead">SoblendWA habla el protocolo de WhatsApp Web sin navegador: WebSocket, Noise, nodos binarios y Signal. Lo usas desde Rust, Node.js, Python o cualquier lenguaje que hable WebSocket o HTTP. Aquí está todo, desde el primer bot hasta correr cincuenta cuentas en un proceso.</p>
  <div class="home-actions">
    <a class="button button-primary" href="#inicio-rapido">Crea tu primer bot</a>
    <a class="button" href="#instalacion">Instalación</a>
    <a class="button" href="https://github.com/Soblend-Development-Studio/SoblendWA">Ver en GitHub</a>
  </div>
</header>
:::

:::tabs
```bash Rust
cargo add soblendwa
cargo add tokio --features full
```
```bash Node.js
npm install soblendwa-node
```
```bash Python
pip install soblendwa
```
```bash Cliente JS
npm install soblendwa-client
```
:::

## Elige por dónde empezar

:::cards
[Crear un bot en 5 minutos](#inicio-rapido) :: Instala, escanea el QR y responde tu primer mensaje.
[Conceptos clave](#conceptos) :: JID, LID, dispositivos y cómo llegan los mensajes.
[Enviar mensajes](#enviar) :: Texto, respuestas, menciones, reacciones, encuestas y más.
[Usar otro lenguaje](#servidor) :: Controla WhatsApp por WebSocket, HTTP o webhooks.
[Migrar sin escanear](#migrar) :: Trae tu sesión de Baileys o de zapo.
[Referencia de la API](#ref-client) :: Cada método público, con su firma y qué hace.
:::

## Rendimiento

El mismo bot con zapo-js 1.11 y con SoblendWA, los dos guardando en SQLite, con 60 ms de red de por medio y subiendo documentos.

:::stats
92 ms :: ida y vuelta en privado (p50), frente a 186 ms
1,6 ms :: de CPU por mensaje, frente a 13,1 ms
63 MB :: de memoria con Node.js incluido, frente a 125 MB
0,7 s :: para arrancar desde la base, frente a 2,9 s
:::

El servidor de WhatsApp es falso y corre en la misma máquina; el método, lo que zapo hace mejor y cómo repetirlo están en [Rendimiento](#rendimiento).

## Antes de usarlo

:::warn Riesgo de bloqueo
SoblendWA es independiente y no está afiliado a WhatsApp ni a Meta. Automatizar una cuenta puede ir contra sus Términos de Servicio y provocar un bloqueo. Lee el [aviso](#aviso) y [qué está probado](#estado).
:::

=== instalacion | Instalación | Empieza
lead: Elige el paquete según el lenguaje de tu bot. Todos usan el mismo motor en Rust.

## Qué paquete necesitas

| Tu bot está en… | Instala | Se ejecuta |
|---|---|---|
| Rust | `cargo add soblendwa` | Dentro de tu programa |
| Node.js o TypeScript | `npm install soblendwa-node` | Módulo nativo, en el proceso de tu bot |
| Python 3.9+ con asyncio | `pip install soblendwa` | Módulo nativo, fuera del GIL |
| JavaScript sin módulos nativos (Bun, Deno, navegador, Alpine) | `npm install soblendwa-client` más `soblendwa-server` | Un servidor aparte |
| Go, PHP, Java, C# o cualquier otro | `soblendwa-server` | Un servidor aparte, por WebSocket, HTTP o webhooks |

No necesitas Rust instalado para usar los paquetes de Node.js, Python ni los binarios del servidor.

## Rust

Necesitas Rust 1.89 o más reciente.

```bash Terminal
cargo add soblendwa
cargo add tokio --features full
```

O en tu `Cargo.toml`, activando las features que quieras:

```toml Cargo.toml
[dependencies]
soblendwa = { version = "{{minor}}", features = ["sqlite"] }
tokio = { version = "1", features = ["full"] }
```

| Feature | Qué añade |
|---|---|
| `sqlite` | `SqliteStore`, con SQLite ya incluido en el binario, y la importación desde zapo |
| `postgres` | `PostgresStore` |
| `redis` | `RedisStore` |
| `mongo` | `MongoStore` y la importación desde `@zapo-js/store-mongo` |
| `stores` | Las cuatro anteriores |
| `testing` | `soblendwa::testing::FakeWhatsApp`, el WhatsApp falso |

Sin features ya tienes `FileStore` y `MemoryStore`.

## Node.js y TypeScript

```bash Terminal
npm install soblendwa-node
```

Funciona con Node.js 18 o más reciente, y los tipos de TypeScript vienen incluidos.

| Sistema | Arquitecturas |
|---|---|
| Windows | x64 |
| macOS | Intel y ARM |
| Linux con glibc 2.35 o más reciente (Ubuntu 22.04, Debian 12) | x64 y ARM64 |

El paquete trae los binarios de las cinco plataformas, así que la descarga ronda los 40 MB. En Alpine y otras distribuciones con musl, usa [el servidor](#servidor) con `soblendwa-client`.

## Python

```bash Terminal
pip install soblendwa
```

Funciona con Python 3.9 o más reciente gracias al ABI estable, en Windows x64, macOS (Intel y ARM) y Linux x64 o ARM64 (manylinux 2.28).

## Cliente JavaScript

```bash Terminal
npm install soblendwa-client
```

No tiene dependencias y funciona en Node.js 18+, Bun, Deno y el navegador. En Node.js 22+, Bun y Deno usa el WebSocket incluido; en Node.js 18 a 21 instala también `ws`. Necesita un `soblendwa-server` corriendo.

## Servidor sin compilar

Cada versión publica binarios en [Releases](https://github.com/Soblend-Development-Studio/SoblendWA/releases). Los de Linux con musl son estáticos: funcionan en cualquier VPS sin instalar nada.

```bash VPS Linux x64
curl -L -o soblendwa.tar.gz https://github.com/Soblend-Development-Studio/SoblendWA/releases/latest/download/soblendwa-server-x86_64-unknown-linux-musl.tar.gz
tar -xzf soblendwa.tar.gz
./soblendwa-server --token "una-clave-larga" --session bot
```

| Archivo | Para |
|---|---|
| `soblendwa-server-x86_64-unknown-linux-musl.tar.gz` | Linux x64, estático |
| `soblendwa-server-aarch64-unknown-linux-musl.tar.gz` | Linux ARM64, estático |
| `soblendwa-server-x86_64-unknown-linux-gnu.tar.gz` | Linux x64 con glibc |
| `soblendwa-server-x86_64-pc-windows-msvc.zip` | Windows x64 |
| `soblendwa-server-aarch64-apple-darwin.tar.gz` | macOS ARM |
| `soblendwa-server-x86_64-apple-darwin.tar.gz` | macOS Intel |

También puedes compilarlo con Rust:

```bash Terminal
cargo install --git https://github.com/Soblend-Development-Studio/SoblendWA soblendwa-server
```

## Compilar los módulos desde el código

```bash Node.js
git clone https://github.com/Soblend-Development-Studio/SoblendWA
cd SoblendWA/bindings/node
npm install
npm run build
npm test
```

```bash Python
cd SoblendWA/bindings/python
pip install maturin
maturin develop --release
```

=== inicio-rapido | Inicio rápido | Empieza
lead: En cinco minutos tendrás un bot que responde a `!ping` en tu propio WhatsApp.

## Lo que necesitas

- Un teléfono con WhatsApp, para escanear el QR. Usa una cuenta de pruebas si puedes.
- Rust, Node.js o Python instalados, según el lenguaje que elijas.

## 1. Escribe el bot

:::tabs
```rust Rust
use soblendwa::{Client, Event, FileStore};

#[tokio::main]
async fn main() -> soblendwa::Result<()> {
    let store = FileStore::open("session").await?;
    let (client, mut events) = Client::builder()
        .store(store)
        .push_name("Mi Bot")
        .build()
        .await?;
    client.connect().await?;

    while let Some(event) = events.recv().await {
        match event {
            Event::Qr(code) => println!("Escanea este QR: {code}"),
            Event::Connected => println!("Conectado como {:?}", client.me()),
            Event::Message(msg) if !msg.info.from_me => {
                if msg.text() == Some("!ping") {
                    let ms = msg.info.elapsed().as_secs_f64() * 1000.0;
                    client.reply(&msg, format!("Pong\n{ms:.2} ms")).await?;
                }
            }
            Event::LoggedOut { reason, .. } => {
                println!("Sesión cerrada: {reason}");
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
```
```js Node.js
const { SoblendWA } = require('soblendwa-node')

async function main() {
  const bot = await SoblendWA.create({ store: { type: 'sqlite', path: 'session/auth.db' } })

  bot.on('qr', ({ code }) => console.log('Escanea este QR:', code))
  bot.on('connected', () => console.log('Conectado como', bot.me))
  bot.on('message', async (message) => {
    if (message.info.fromMe) return
    if (message.text === '!ping') await bot.reply(message, `Pong\n${bot.elapsed(message).toFixed(2)} ms`)
  })

  await bot.connect()
}

main()
```
```python Python
import asyncio
from soblendwa import SoblendWA


async def main():
    bot = await SoblendWA.create(store={"type": "sqlite", "path": "session/auth.db"})

    @bot.on("qr")
    def show_qr(event):
        print("Escanea este QR:", event["code"])

    @bot.on("message")
    async def answer(message):
        if message["info"]["fromMe"]:
            return
        if message["text"] == "!ping":
            await bot.reply(message, f"Pong\n{bot.elapsed(message):.2f} ms")

    await bot.run()


asyncio.run(main())
```
```js Cliente JS
import { connect } from 'soblendwa-client'

const bot = await connect('ws://127.0.0.1:8787', { session: 'bot', token: 'una-clave-larga' })

bot.on('qr', ({ code }) => console.log('Escanea este QR:', code))
bot.on('message', async (message) => {
  if (message.info.fromMe) return
  if (message.text === '!ping') {
    const ms = await bot.elapsed(message)
    await bot.reply(message, `Pong\n${ms.toFixed(2)} ms`)
  }
})
```
:::

Si usas el cliente JS, primero arranca el servidor con `soblendwa-server --token "una-clave-larga" --session bot`. El servidor dibuja el QR en la terminal.

## 2. Vincula tu teléfono

El texto del evento `qr` es el contenido del código QR, no una imagen. Conviértelo en imagen con cualquier librería de QR, como `qrcode-terminal` en Node.js o `qrcode` en Python, o usa un [código de 8 caracteres](#vincular-codigo).

:::steps
- Ejecuta el bot.
- En el teléfono, abre **WhatsApp > Dispositivos vinculados > Vincular un dispositivo**.
- Escanea el QR. El bot se reinicia solo una vez y emite `connected`.
- Desde **otro número**, envía `!ping` a la cuenta vinculada.
- Recibes `Pong` y el tiempo que tardó el bot.
:::

:::note Mensajes viejos
Al conectar te pueden llegar mensajes de cuando el bot estaba apagado. Para no responder comandos de hace horas, ignóralos con [`is_stale`](#recibir-viejos).
:::

## 3. Qué acaba de pasar

Tu bot abrió un WebSocket con WhatsApp, verificó su certificado y se registró como un dispositivo vinculado más, igual que WhatsApp Web. Las claves se guardaron en la carpeta o base de datos que elegiste, así que la próxima vez conectará sin QR.

Todo lo que ocurre llega como un [evento](#ref-eventos). Tu bot reacciona a los que le interesan y llama métodos para actuar.

## Siguientes pasos

:::cards
[Enviar de todo](#enviar) :: Menciones, reacciones, ediciones, ubicación y contactos.
[Multimedia](#multimedia) :: Imágenes, audio, video, documentos y archivos enormes.
[Botones y listas](#interactivos) :: Los mensajes interactivos que WhatsApp sí muestra.
[Prueba sin arriesgar tu número](#pruebas) :: El WhatsApp falso, con el protocolo real.
:::

=== vincular | Vincular y sesión | Empieza
lead: Cómo se vincula una cuenta, cuánto dura la sesión y qué pasa cuando se corta la conexión.

## Vincular con QR

Después de `connect()`, si la sesión no está vinculada, llega un evento `Qr` con el contenido del código. El primero dura 60 segundos y los siguientes 20. Mientras no se escanee, llegan códigos nuevos. Si se agotan, la conexión termina con el motivo `QrTimeout` y, con la reconexión automática activada, empieza otra tanda de códigos.

Al escanearlo llegan, en este orden:

| Evento | Qué significa |
|---|---|
| `PairSuccess { id, lid, platform, business_name }` | El teléfono aprobó la vinculación |
| `Disconnected { reason: RestartRequired, reconnecting: true }` | Reinicio normal que pide WhatsApp tras vincular. No es un error |
| `Connected` | La sesión está lista para enviar y recibir |

## Vincular con código {#vincular-codigo}

Sirve cuando no puedes escanear, por ejemplo en un servidor sin pantalla. Pide el código después del primer evento `qr` y escríbelo en **Dispositivos vinculados > Vincular con el número de teléfono**.

:::tabs
```rust Rust
if let Event::Qr(_) = event {
    let code = client.request_pair_code("5215512345678").await?;
    println!("Escribe este código en tu teléfono: {code}");
}
```
```js Node.js
bot.once('qr', async () => {
  const code = await bot.requestPairCode('5215512345678')
  console.log('Escribe este código en tu teléfono:', code)
})
```
```python Python
@bot.on("qr")
async def pair(event):
    print("Código:", await bot.request_pair_code("5215512345678"))
```
```js Cliente JS
bot.once('qr', async () => {
  console.log('Código:', await bot.requestPairCode('5215512345678'))
})
```
:::

Escribe el número con código de país y sin espacios. También hay un ejemplo listo: `cargo run --release --example pair_code -- 5215512345678`.

## Cuánto dura la sesión

La sesión queda en el [almacenamiento](#almacenamiento) que elegiste y sobrevive a los reinicios. Termina solo si cierras la vinculación desde el teléfono, si llamas a `logout()` o si WhatsApp la invalida. En esos casos llega `LoggedOut { reason, cause, code }`, los datos se borran y el siguiente `connect()` muestra un QR nuevo.

## Por qué se cerró la sesión {#vincular-causa}

`cause` dice qué pasó, para que tu panel pueda distinguirlo sin leer textos.

| `cause` en Rust | En JSON | Qué pasó | `code` |
|---|---|---|---|
| `RemovedFromPhone` | `removedFromPhone` | El teléfono quitó este dispositivo de **Dispositivos vinculados** | `401` |
| `SessionInvalidated` | `sessionInvalidated` | WhatsApp dejó de aceptar la sesión guardada: por ejemplo, se reinstaló WhatsApp en el teléfono o la vinculación caducó | `401` o `419` |
| `UserLogout` | `userLogout` | Tu bot llamó a `logout()` | — |

:::note Lo que WhatsApp no dice
Cuando el teléfono quita un dispositivo, WhatsApp no dice si lo hizo el dueño a mano o si fue el teléfono por su cuenta. `removedFromPhone` cubre los dos casos. Un bloqueo de la cuenta no llega como cierre de sesión: llega como [`accountBlocked`](#confiabilidad-bloqueo), y la sesión se conserva.
:::

:::tabs
```rust Rust
use soblendwa::events::LogoutCause;

if let Event::LoggedOut { cause, reason, .. } = &event {
    if *cause == LogoutCause::RemovedFromPhone {
        println!("Quitaron este bot desde el teléfono: {reason}");
    }
}
```
```js Node.js
bot.on('loggedOut', ({ cause, reason }) => {
  if (cause === 'removedFromPhone') console.log('Quitaron este bot desde el teléfono:', reason)
})
```
```python Python
@bot.on("loggedOut")
def ended(event):
    if event["cause"] == "removedFromPhone":
        print("Quitaron este bot desde el teléfono:", event["reason"])
```
```js Cliente JS
bot.on('loggedOut', ({ cause, reason }) => {
  if (cause === 'removedFromPhone') console.log('Quitaron este bot desde el teléfono:', reason)
})
```
:::

:::danger Protege tu carpeta de sesión
Contiene las claves de tu cuenta. Quien la tenga puede enviar y leer mensajes en tu nombre, así que nunca la subas a Git ni la compartas.
:::

:::warn Una sesión, un proceso
Dos procesos no pueden usar la misma sesión a la vez. El segundo expulsa al primero, que recibe el motivo `Replaced`, y uno puede consumir los mensajes pendientes del otro. Detén el bot antes de arrancar otra copia.
:::

## Motivos de desconexión

Cada desconexión llega como `Disconnected { reason, reconnecting }`.

| Motivo en Rust | En JSON | Qué significa | ¿Reintenta? |
|---|---|---|---|
| `ConnectionLost(texto)` | `connectionLost` | Se cayó la red o el socket | Sí, si `auto_reconnect` está activo |
| `RestartRequired` | `restartRequired` | WhatsApp pide reiniciar, normal tras vincular | Sí, siempre |
| `Replaced` | `replaced` | Otra conexión tomó esta sesión | No |
| `QrTimeout` | `qrTimeout` | Nadie escaneó a tiempo | Sí, si `auto_reconnect` está activo, con QR nuevos |
| `ClientOutdated` | `clientOutdated` | WhatsApp rechazó la versión del cliente (código 405) | Sí, siempre, tras olvidar la versión guardada |
| `AccountBlocked(código)` | `accountBlocked` | [Cuenta bloqueada](#confiabilidad-bloqueo) | No |
| `Manual` | `manual` | Llamaste a `disconnect()` | No |

La espera entre reintentos está explicada en [Confiabilidad](#confiabilidad-reconexion).

## Cerrar la conexión

| Método | Qué hace | ¿Conserva la sesión? |
|---|---|---|
| `client.disconnect()` | Cierra el socket y guarda lo pendiente en el almacenamiento | Sí |
| `client.logout()` | Le pide a WhatsApp desvincular el dispositivo y borra los datos | No |

## Estado de la conexión

| Método | Devuelve |
|---|---|
| `client.is_connected()` | Si el socket está conectado y autenticado |
| `client.is_logged_in()` | Si hay una cuenta vinculada |
| `client.me()` | Tu JID de número, con dispositivo |
| `client.lid()` | Tu LID |
| `client.latency()` | La latencia de red medida en segundo plano |

=== conceptos | Conceptos | Empieza
lead: Las pocas ideas que necesitas para entender todo lo demás.

## Dispositivo vinculado

SoblendWA es un dispositivo más de tu cuenta, como WhatsApp Web. No se registra como teléfono: necesita un teléfono real que apruebe la vinculación, con WhatsApp instalado.

## JID: la dirección de todo {#conceptos-jid}

Cada cuenta, grupo o canal tiene un JID con la forma `usuario:dispositivo@servidor`.

| Ejemplo | Qué es | Cómo crearlo en Rust |
|---|---|---|
| `5215512345678@s.whatsapp.net` | Una cuenta, por su número | `Jid::phone("5215512345678")` |
| `5215512345678:3@s.whatsapp.net` | El dispositivo 3 de esa cuenta | `jid.with_device(3)` |
| `100000000000001@lid` | Una cuenta, por su LID | `Jid::lid("100000000000001")` |
| `120363000000000001@g.us` | Un grupo | `Jid::group("120363000000000001")` |
| `120363000000000001@newsletter` | Un canal | `Jid::new("120363000000000001", "newsletter")` |
| `status@broadcast` | Los estados | `Jid::status_broadcast()` |

Un JID también se puede leer desde un texto:

```rust
let jid: Jid = "5215512345678@s.whatsapp.net".parse()?;
let same = jid.same_user(&Jid::phone("52 1 55 1234 5678"));
```

`Jid::phone` conserva solo los dígitos, así que acepta números con espacios o signos. En Node.js, Python y el servidor, cualquier parámetro `chat` acepta un JID completo o solo el número.

## LID: cuando WhatsApp oculta el número {#conceptos-lid}

WhatsApp identifica a muchas cuentas con un LID, un identificador que no revela el teléfono, sobre todo en grupos. Una misma persona puede aparecer por su número o por su LID.

SoblendWA resuelve ambos por ti. Cada mensaje trae `info.sender_phone` e `info.sender_lid`, y `group.is_admin(&sender)` funciona con cualquiera de los dos. Si WhatsApp no comparte el número de alguien, `sender_phone` queda vacío; el LID sigue sirviendo para identificarlo.

Puedes consultar la relación en cualquier momento, y te avisa cuando descubre un par nuevo, así puedes unir perfiles en tu base sin migraciones.

:::tabs
```rust Rust
let lid = client.lid_for_phone(&Jid::phone("5215511111111")).await?;
if let Event::LidMapping { phone, lid } = event {
    println!("{phone} y {lid} son la misma persona");
}
```
```js Node.js
const lid = await bot.lidForPhone('5215511111111')
const phone = await bot.phoneForLid('100000000000001@lid')
bot.on('lidMapping', ({ phone, lid }) => console.log(phone, 'y', lid, 'son la misma persona'))
```
```python Python
lid = await bot.lid_for_phone("5215511111111")
phone = await bot.phone_for_lid("100000000000001@lid")

@bot.on("lidMapping")
def linked(event):
    print(event["phone"], "y", event["lid"], "son la misma persona")
```
```js Cliente JS
const lid = await bot.lidForPhone('5215511111111')
bot.on('lidMapping', ({ phone, lid }) => console.log(phone, 'y', lid, 'son la misma persona'))
```
:::

`lidMapping` llega una sola vez por par, la primera vez que se conoce. Al vincular puede llegar muchas veces seguidas, una por cada contacto del historial.

## Chat y remitente

`info.chat` es la conversación y `info.sender` es quien escribió. En un chat privado suelen coincidir; en un grupo, `chat` es el grupo y `sender` es la persona. Responde siempre a `chat`.

## Identificador y clave de un mensaje

Cada mensaje tiene un `id` de texto. Para editarlo, borrarlo o fijarlo necesitas su clave (`MessageKey`): el chat, el id, si es tuyo y, en grupos, quién lo envió. `msg.key()` te la da hecha.

## Mensajes envueltos

WhatsApp mete los mensajes dentro de otros: `ephemeral`, `viewOnce`, `documentWithCaption`, hijos de álbum, Meta AI, estados de grupo. SoblendWA los desenvuelve, así que `msg.text()`, `msg.message.media()` y `msg.quoted()` funcionan igual en todos. Si necesitas el original, está en `msg.raw`.

## Eventos

Todo lo que ocurre llega por un único canal de eventos: mensajes, recibos, cambios de grupo, desconexiones. En Rust es un `enum`, y en los demás lenguajes es JSON con un campo `type`. La lista completa está en [Eventos](#ref-eventos).

## Colas por chat

Los chats distintos se atienden en paralelo y cada chat conserva su orden, al recibir y al enviar. Un texto nunca espera detrás de un video pesado de otro chat, y en el mismo chat los mensajes salen en el orden en que los pediste.

En Node.js, Python y por WebSocket el orden se fija en el momento de la llamada, así que `bot.sendText(chat, 'uno'); bot.sendText(chat, 'dos')` sin `await` sale en ese orden. Cada llamada toma un turno en su chat; mientras espera no gasta CPU, y al terminar despierta solo a la siguiente.

Los archivos empiezan a subir en cuanto los pides, sin esperar turno; lo que espera es la entrega. Así varias fotos al mismo chat suben a la vez y llegan en orden.

## Sesiones, cuentas y almacenamiento

Una sesión es una cuenta vinculada con sus claves, guardadas en un [store](#almacenamiento). Puedes tener varias en el mismo proceso, cada una con su propio cliente. El [servidor](#servidor) maneja varias a la vez por nombre.
