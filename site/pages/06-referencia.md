=== ref-client | Client | Referencia de la API
lead: Cada método público del cliente de Rust. En esta página, `async` indica que se usa con `.await`.

La página también está en el buscador: pulsa `Ctrl K` y escribe el nombre de un método.

## Crear y conectar

@fn Client builder :: Empieza a [configurar](#ref-builder) un cliente.
@fn Client connect :: Abre la conexión y vuelve al instante. El resultado llega por eventos. Mantiene la conexión viva y reconecta en segundo plano.
@fn Client disconnect :: Cierra la conexión y guarda lo pendiente en el almacenamiento. Conserva la sesión.
@fn Client logout :: Le pide a WhatsApp desvincular este dispositivo y borra todos los datos de la sesión.
@fn Client request_pair_code :: Pide un [código de 8 caracteres](#vincular-codigo) para vincular sin QR. Llámalo después del primer `Event::Qr`.

## Estado

@fn Client is_connected :: Indica si la sesión está conectada y lista.
@fn Client is_logged_in :: Indica si hay una cuenta vinculada.
@fn Client me :: Tu JID de número, con dispositivo.
@fn Client lid :: Tu LID.
@fn Client creds :: Una copia de las credenciales. Contiene claves privadas: no las muestres en registros.
@fn Client config :: La configuración con la que se creó.
@fn Client store :: El almacenamiento en uso.
@fn Client chats :: El [store de chats](#store-chats), si está activado.
@fn Client stats :: Las [métricas](#confiabilidad-metricas) de la sesión.
@fn Client pending_sends :: Los envíos del [outbox](#confiabilidad-outbox) esperando confirmación.
@fn Client ping :: Mide la latencia de red ahora. Tarda un viaje de ida y vuelta.
@fn Client latency :: La última latencia medida en segundo plano. Responde al instante.
@fn Client wa_version :: La versión de WhatsApp Web en uso.

## Enviar

@fn Client send_message :: Envía cualquier [`Message`](#ref-message). Devuelve el id. Es la base de los demás envíos.
@fn Client send_message_with :: Como `send_message`, con [`SendOptions`](#enviar-opciones).
@fn Client send_text :: Envía texto. Si trae un enlace, añade la vista previa.
@fn Client reply :: Responde a un mensaje, citándolo, en el mismo chat.
@fn Client react :: Reacciona a un mensaje. Con un emoji vacío, quita la reacción.
@fn Client edit_message :: Reemplaza el contenido de un mensaje tuyo.
@fn Client revoke_message :: Elimina un mensaje para todos.
@fn Client pin_message :: Fija un mensaje durante los segundos indicados.
@fn Client unpin_message :: Desfija un mensaje.
@fn Client send_album :: Envía varias fotos o videos ya preparados como un álbum. Devuelve el id del álbum y el de cada elemento.
@fn Client send_status :: Publica un estado para esos contactos.
@fn Client send_group_invite :: Envía una tarjeta de invitación a un grupo. Requiere ser administrador.
@fn Client send_product :: Envía un producto de tu [catálogo](#empresa). {exp}
@fn Client set_member_label :: Pone tu etiqueta en un grupo, de hasta 30 caracteres. {exp}

## Vista previa de enlaces

@fn Client link_preview :: Obtiene los datos de un enlace sin enviar nada. Usa la caché y tu fuente propia si la configuraste.
@fn Client with_link_preview :: Devuelve el mensaje con su vista previa añadida, si tiene un enlace.
@fn Client send_text_with_preview :: Envía un texto pidiendo la vista previa de forma explícita.
@fn Client set_link_previews_disabled :: Cambia el ajuste de privacidad de vistas previas de tu cuenta.

## Multimedia

@fn Client send_media :: Sube y envía con [`MediaOptions`](#multimedia-opciones).
@fn Client send_image :: Envía una imagen, con caption opcional.
@fn Client send_video :: Envía un video, con caption opcional.
@fn Client send_audio :: Envía audio. Con `true`, como nota de voz.
@fn Client send_document :: Envía un documento con su nombre y tipo MIME.
@fn Client send_sticker :: Envía un sticker en WebP.
@fn Client prepare_media :: Sube el archivo y devuelve un `Message` listo para enviar.
@fn Client send_prepared :: Toma su lugar en el chat al llamarla y envía el `Message` de un future que sigue corriendo mientras salen los anteriores.
@fn Client upload :: Cifra y sube los bytes sin enviarlos. Reutiliza lo ya subido.
@fn Client download :: Descarga y verifica cualquier multimedia.
@fn Client download_message :: Descarga la multimedia de un `Message`.
@fn Client download_event :: Descarga la de un mensaje recibido. Si caducó, pide que se vuelva a subir.
@fn Client reupload_media :: Le pide al remitente que vuelva a subir un archivo caducado. {exp}
@fn Client forget_upload :: Olvida un archivo de la caché de multimedia.

## Archivos grandes

@fn Client send_media_file :: Envía un archivo del disco por bloques, con memoria constante.
@fn Client prepare_media_file :: Como `prepare_media`, desde una ruta.
@fn Client upload_file :: Sube un archivo del disco por bloques.
@fn Client upload_reader :: Sube desde un lector de un solo uso, cifrando hacia un archivo temporal.
@fn Client download_to_file :: Descarga a disco por bloques. El archivo solo aparece con su nombre si pasa la verificación.
@fn Client download_event_to_file :: Como `download_to_file`, para un mensaje recibido. Si caducó, pide que se vuelva a subir.
@fn Client download_to_writer :: Descarga hacia cualquier escritor. Devuelve los bytes escritos.

## Recibos

@fn Client read_message :: Marca un mensaje recibido como leído.
@fn Client mark_read :: Marca varios mensajes de un chat como leídos.
@fn Client mark_played :: Marca una nota de voz o un video circular como reproducidos.
@fn Client send_receipt :: Envía un recibo de cualquier tipo.

## Encuestas y eventos

@fn Client vote_poll :: Vota en una encuesta con las opciones elegidas, por nombre.
@fn Client respond_event :: Responde a un evento de calendario, con invitados extra opcionales.

## Grupos

@fn Client group_info :: Datos de un grupo, desde la caché si los tiene.
@fn Client group_metadata :: Datos de un grupo, siempre desde el servidor.
@fn Client joined_groups :: Todos tus grupos.
@fn Client create_group :: Crea un grupo.
@fn Client update_participants :: Añade, saca, nombra o quita administradores. Devuelve el resultado de cada persona.
@fn Client set_group_subject :: Cambia el nombre.
@fn Client set_group_description :: Cambia la descripción. Con `None`, la quita.
@fn Client set_group_announce :: Decide si solo los administradores pueden escribir.
@fn Client set_group_locked :: Decide si solo los administradores pueden editar los datos.
@fn Client set_group_ephemeral :: Activa los mensajes temporales. `0` los desactiva.
@fn Client set_group_picture :: Cambia la foto. La imagen se recorta y redimensiona sola.
@fn Client set_group_join_approval :: Decide si las solicitudes para entrar necesitan aprobación.
@fn Client set_group_member_add_mode :: Decide si todos los miembros pueden añadir gente.
@fn Client leave_group :: Sale del grupo.
@fn Client group_invite_code :: El código de invitación.
@fn Client revoke_group_invite :: Invalida el código y devuelve uno nuevo.
@fn Client join_group :: Entra con un código o enlace de invitación.
@fn Client group_invite_info :: Datos de un grupo a partir de su invitación, sin entrar.
@fn Client accept_group_invite :: Acepta una tarjeta de invitación recibida por mensaje.
@fn Client group_join_requests :: Las solicitudes pendientes.
@fn Client respond_join_requests :: Aprueba o rechaza solicitudes.

## Comunidades {exp}

@fn Client create_community :: Crea una comunidad.
@fn Client create_community_group :: Crea un grupo dentro de una comunidad.
@fn Client link_group :: Vincula un grupo existente.
@fn Client unlink_group :: Desvincula un grupo.
@fn Client community_groups :: Los grupos de una comunidad.
@fn Client leave_community :: Sale de la comunidad.
@fn Client set_community_join_approval :: Decide si entrar necesita aprobación.

## Canales

@fn Client newsletter_create :: Crea un canal. {exp}
@fn Client newsletter_info :: Datos de un canal por su JID.
@fn Client newsletter_info_by_invite :: Datos de un canal por su enlace o código.
@fn Client newsletter_follow :: Sigue un canal.
@fn Client newsletter_unfollow :: Deja de seguirlo.
@fn Client newsletter_mute :: Lo silencia o no.
@fn Client newsletter_update :: Cambia el nombre, la descripción o la foto. {exp}
@fn Client newsletter_delete :: Borra un canal tuyo. {exp}
@fn Client newsletter_subscribers :: Número de suscriptores.
@fn Client newsletter_admin_count :: Número de administradores.
@fn Client newsletter_change_owner :: Cambia el dueño. {exp}
@fn Client newsletter_demote :: Le quita el rol de administrador a alguien. {exp}
@fn Client newsletter_react :: Reacciona a una publicación por su `server_id`. Con `None`, quita la reacción.
@fn Client newsletter_messages :: Lee publicaciones, opcionalmente anteriores a un `server_id`.
@fn Client newsletter_live_updates :: Pide recibir los cambios en vivo. Devuelve por cuántos segundos, si WhatsApp lo indica.

## Chats y app state

@fn Client archive_chat :: Archiva o desarchiva.
@fn Client pin_chat :: Fija o desfija.
@fn Client mute_chat :: Silencia por ese tiempo. Con `None`, quita el silencio.
@fn Client mark_chat_read :: Marca el chat como leído o no leído.
@fn Client delete_chat :: Borra el chat.
@fn Client clear_chat :: Vacía el chat.
@fn Client star_message :: Destaca un mensaje o le quita el destacado.
@fn Client save_contact :: Guarda un contacto.
@fn Client chat_modify :: Aplica cualquier [`ChatModification`](#chats-cualquier-otro-cambio).
@fn Client set_push_name :: Cambia tu nombre visible.
@fn Client edit_label :: Crea o edita una etiqueta. {exp}
@fn Client delete_label :: Borra una etiqueta. {exp}
@fn Client label_chat :: Pone o quita una etiqueta en un chat. {exp}
@fn Client label_message :: Pone o quita una etiqueta en un mensaje. {exp}
@fn Client save_quick_reply :: Crea o edita una respuesta rápida. Devuelve su id. {exp}
@fn Client delete_quick_reply :: Borra una respuesta rápida. {exp}
@fn Client sync_app_state :: Fuerza la sincronización de esas colecciones.

## Historial

@fn Client request_history :: Le pide al teléfono hasta 50 mensajes anteriores. Llegan como `Event::HistorySync`.
@fn Client request_placeholder_resend :: Le pide al teléfono una copia de mensajes que no se pudieron descifrar.
@fn Client download_history :: Descarga y descomprime un bloque de historial.

## Perfil y privacidad

@fn Client profile_picture_url :: URL de una foto de perfil. Con `true`, en tamaño completo.
@fn Client set_profile_picture :: Cambia tu foto, o la de un grupo si indicas su JID.
@fn Client remove_profile_picture :: Quita esa foto.
@fn Client set_about :: Cambia tu información.
@fn Client fetch_about :: Lee la información de varias cuentas.
@fn Client block :: Bloquea una cuenta.
@fn Client unblock :: Desbloquea una cuenta.
@fn Client blocklist :: Tu lista de bloqueados.
@fn Client privacy_settings :: Tus ajustes de privacidad.
@fn Client set_privacy :: Cambia un ajuste de privacidad.
@fn Client set_default_disappearing :: Duración por defecto de los mensajes temporales. `0` los desactiva.
@fn Client set_disappearing :: Duración de los mensajes temporales en un chat.
@fn Client disappearing_modes :: Los mensajes temporales de varios chats.
@fn Client business_profile :: El perfil de empresa de una cuenta, si tiene.

## Cuentas

@fn Client on_whatsapp :: Comprueba si unos números tienen WhatsApp.
@fn Client lid_for_phone :: El LID de un número.
@fn Client phone_for_lid :: El número de un LID, si WhatsApp lo compartió.
@fn Client bot_list :: Los bots de Meta AI disponibles.
@fn Client reachout_timelock :: Si WhatsApp limitó el contacto con gente nueva.
@fn Client new_chat_message_cap :: Cuántos chats nuevos puedes iniciar, como JSON.

## Presencia y llamadas

@fn Client set_presence :: Aparece en línea o no.
@fn Client subscribe_presence :: Pide recibir la presencia de una cuenta.
@fn Client send_chat_presence :: Envía escribiendo, grabando o pausa.
@fn Client create_call_link :: Crea un enlace de llamada. Con `true`, de video.
@fn Client reject_call :: Rechaza una llamada entrante.

## Empresa {exp}

@fn Client catalog :: Una página de un catálogo.
@fn Client catalog_collections :: Las colecciones de un catálogo.
@fn Client order_details :: Los datos de un pedido.
@fn Client product_create :: Crea un producto.
@fn Client product_update :: Edita un producto.
@fn Client product_delete :: Borra productos. Devuelve cuántos.
@fn Client update_business_profile :: Cambia tu perfil de empresa.
@fn Client set_business_cover_photo :: Pone una foto de portada. Devuelve su id.
@fn Client remove_business_cover_photo :: Quita la foto de portada.

## Bajo nivel

Para lo que la librería aún no cubre. Un error aquí puede hacer que WhatsApp cierre la conexión.

@fn Client send_node :: Envía un nodo binario sin esperar respuesta.
@fn Client query :: Envía un nodo y espera la respuesta. Añade un id si no lo tiene.
@fn Client generate_tag :: Genera un id para un nodo.
@fn Client rotate_signed_pre_key :: Rota la signed pre-key. Normalmente ocurre sola, cada 30 días.
@fn Client clean_dirty_bits :: Le avisa a WhatsApp que ya atendiste una señal de sincronización.

=== ref-builder | ClientBuilder | Referencia de la API
lead: Configura el cliente antes de crearlo. Los valores por defecto están en [Configuración](#configuracion).

@fn ClientBuilder store :: Dónde se guarda la sesión. Por defecto, memoria.
@fn ClientBuilder chat_store :: Activa el [store de chats](#store-chats).
@fn ClientBuilder config :: Reemplaza toda la configuración.
@fn ClientBuilder device_name :: Nombre en Dispositivos vinculados.
@fn ClientBuilder platform :: La plataforma que se informa a WhatsApp.
@fn ClientBuilder version :: Fija la versión de WhatsApp Web.
@fn ClientBuilder push_name :: Tu nombre visible.
@fn ClientBuilder country_code :: El país que se informa a WhatsApp.
@fn ClientBuilder online_on_connect :: Aparecer en línea al conectar.
@fn ClientBuilder auto_reconnect :: Reconectar solo.
@fn ClientBuilder query_timeout :: Tiempo máximo de una consulta.
@fn ClientBuilder connect_timeout :: Tiempo máximo para conectar.
@fn ClientBuilder keepalive :: Cada cuánto se comprueba que la conexión sigue viva.
@fn ClientBuilder media_cache :: Cuánto se recuerda la multimedia. Con `None`, se desactiva.
@fn ClientBuilder send_limit :: El [límite de envíos](#confiabilidad-limite). Con `None`, se desactiva.
@fn ClientBuilder outbox :: El [outbox persistente](#confiabilidad-outbox).
@fn ClientBuilder link_previews :: La [vista previa de enlaces](#vista-previa).
@fn ClientBuilder proxy :: El [proxy](#proxy).
@fn ClientBuilder workers :: Cuántos mensajes se procesan en paralelo.
@fn ClientBuilder media_store :: Un store para la [multimedia compartida](#almacenamiento-multimedia-compartida) entre cuentas.
@fn ClientBuilder retry_deadline :: Cuánto esperar un mensaje que no se pudo descifrar antes de emitir `MessageLost`.
@fn ClientBuilder ws_url :: Conecta a otro servidor, como uno falso.
@fn ClientBuilder media_endpoint :: Otro servidor de multimedia.
@fn ClientBuilder trust_root :: Raíz de certificados de ese servidor.
@fn ClientBuilder build :: Crea el cliente y devuelve también el canal de eventos.

=== ref-message | Message | Referencia de la API
lead: Constructores y ayudas de `Message`, el protobuf de WhatsApp. Sus campos son públicos, así que puedes construir cualquier mensaje.

`Message` admite `From<&str>` y `From<String>`, por eso `client.reply(&msg, "hola")` funciona.

## Crear

@fn Message text :: Un texto.
@fn Message reaction :: Una reacción a un mensaje por su clave.
@fn Message location :: Una ubicación.
@fn Message contact :: Un contacto, con su vCard.
@fn Message contacts :: Varios contactos.
@fn Message vcard :: Genera el texto vCard de un contacto.
@fn Message poll :: Una encuesta. Con `selectable` igual a `1`, de opción única.
@fn Message announcement_poll :: Una encuesta para grupos de anuncios.
@fn Message event :: Un evento de calendario.
@fn Message event_details :: Añade descripción, fin, lugar, enlace e invitados a un evento.
@fn Message cancel_event :: Marca un evento como cancelado.
@fn Message group_invite :: Una tarjeta de invitación con su código. `send_group_invite` lo hace por ti.
@fn Message product :: Un mensaje de producto.
@fn Message share_phone_number :: Comparte tu número en el chat.
@fn Message request_phone_number :: Pide el número de la otra persona.
@fn Message limit_sharing :: Activa o desactiva el límite de reenvío.
@fn Message member_label :: Una etiqueta de miembro, de hasta 30 caracteres.
@fn Message video_note :: Convierte un video preparado en video circular.
@fn Message pin :: Fija o desfija un mensaje por su clave.
@fn Message revoke :: Elimina un mensaje por su clave.
@fn Message edit :: Edita un mensaje por su clave.

## Interactivos

@fn Message buttons :: Botones simples, en formato *native flow*.
@fn Message list :: Una lista de opciones, en formato *native flow*.
@fn Message template :: Un mensaje de plantilla.
@fn Message legacy_buttons :: El formato viejo de botones. WhatsApp ya no lo muestra en cuentas normales.
@fn Message legacy_list :: El formato viejo de listas. WhatsApp ya no lo muestra en cuentas normales.
@fn Message button_reply :: Una respuesta a un botón.
@fn Message template_reply :: Una respuesta a una plantilla.
@fn Message list_reply :: Una respuesta a una lista.

## Modificar

@fn Message reply_to :: Cita el mensaje indicado.
@fn Message mention :: Añade los JID mencionados.
@fn Message mention_all :: Menciona a todo el grupo.
@fn Message forwarded :: Lo marca como reenviado.
@fn Message context_info_mut :: Acceso de escritura al `ContextInfo`.

## Leer

@fn Message get_text :: El texto o el caption, sin importar el tipo.
@fn Message unwrapped :: El mensaje real, quitando los envoltorios.
@fn Message media :: La multimedia, lista para descargar.
@fn Message media_type :: El tipo de multimedia.
@fn Message stanza_type :: El tipo de mensaje.
@fn Message is_view_once :: Si es de una sola vista.
@fn Message has_content :: Si tiene algún contenido.
@fn Message context_info :: El `ContextInfo`: lo citado y las menciones.
@fn Message interactive_reply :: La opción elegida, si responde a un botón o lista.
@fn Message link_thumbnail :: Los datos para descargar la miniatura nítida de una vista previa.

## Mensajes recibidos

@fn MessageEvent text :: El texto o el caption.
@fn MessageEvent key :: La clave, para editar, borrar o fijar.
@fn MessageEvent chat :: El chat.
@fn MessageEvent sender :: El remitente.
@fn MessageEvent is_reaction :: Si es una reacción.
@fn MessageEvent quoted :: El mensaje citado.
@fn MessageEvent mentions :: Los JID mencionados.
@fn MessageEvent to_json :: El mensaje en JSON, como en los demás lenguajes.
@fn MessageInfo elapsed :: Cuánto lleva dentro del bot.
@fn MessageInfo is_older_than :: Si es más viejo que esa duración.
@fn MessageInfo is_stale :: Si llegó con el bot apagado o es más viejo que esa duración.
@fn MessageInfo key :: La clave del mensaje.
@fn MessageInfo to_json :: Los datos en JSON.

## Botones

@fn Interactive new :: Empieza un mensaje interactivo con su texto.
@fn Interactive footer :: Añade el pie.
@fn Interactive title :: Añade el título.
@fn Interactive subtitle :: Añade el subtítulo.
@fn Interactive image :: Pone una imagen preparada en la cabecera.
@fn Interactive video :: Pone un video preparado en la cabecera.
@fn Interactive document :: Pone un documento preparado en la cabecera.
@fn Interactive button :: Añade cualquier `NativeButton`.
@fn Interactive quick_reply :: Añade una respuesta rápida.
@fn Interactive url :: Añade un botón que abre un enlace.
@fn Interactive copy :: Añade un botón que copia un código.
@fn Interactive call :: Añade un botón de llamada.
@fn Interactive select :: Añade un menú de opciones.
@fn Interactive build :: Construye el `Message`.
@fn Section new :: Empieza una sección con su título.
@fn Section row :: Añade una fila.
@fn Section described_row :: Añade una fila con descripción.

=== ref-chatstore | ChatStore | Referencia de la API
lead: El [store de chats](#store-chats): chats, contactos y mensajes guardados en local.

@fn ChatStore new :: Un store solo en memoria.
@fn ChatStore with_limit :: Un store en memoria con otro límite de mensajes por chat.
@fn ChatStore open :: Abre o crea el archivo. Guarda hasta 500 mensajes por chat.
@fn ChatStore open_with_limit :: Abre o crea el archivo con otro límite.
@fn ChatStore save :: Escribe a disco ahora, de forma atómica.
@fn ChatStore save_to :: Escribe en otra ruta.
@fn ChatStore chats :: Todos los chats.
@fn ChatStore chat :: Un chat.
@fn ChatStore unread_total :: Total de mensajes sin leer.
@fn ChatStore contacts :: Todos los contactos.
@fn ChatStore contact :: Un contacto.
@fn ChatStore name_of :: El mejor nombre conocido de un JID.
@fn ChatStore messages :: Los últimos mensajes de un chat.
@fn ChatStore messages_before :: Los mensajes anteriores a esa hora.
@fn ChatStore message :: Un mensaje por chat e id.
@fn ChatStore find_message :: Un mensaje por id, en todos los chats.
@fn ChatStore search :: Busca texto en todos los mensajes.
@fn ChatStore ingest :: Procesa un evento. El cliente lo hace solo si usaste `.chat_store(...)`.
@fn Contact display_name :: El mejor nombre del contacto.
@fn StoredMessage text :: El texto o el caption.
@fn StoredMessage to_event :: Lo convierte en un `MessageEvent`, por ejemplo para responderlo con `reply`.
@fn StoredMessage to_json :: El mensaje en JSON.

=== ref-jid | Jid | Referencia de la API
lead: La [dirección](#conceptos-jid) de una cuenta, grupo o canal.

`Jid` tiene tres campos públicos: `user`, `device` y `server`. También implementa `Display` y `FromStr`, así que puedes usar `to_string()` y `parse()`.

@fn Jid new :: Un JID con esos usuario y servidor.
@fn Jid phone :: Una cuenta por su número. Conserva solo los dígitos.
@fn Jid group :: Un grupo por su id.
@fn Jid lid :: Una cuenta por su LID.
@fn Jid server :: El servidor de WhatsApp, `s.whatsapp.net`.
@fn Jid group_server :: El servidor de grupos, `g.us`.
@fn Jid status_broadcast :: Los estados, `status@broadcast`.
@fn Jid with_device :: El mismo JID en ese dispositivo.
@fn Jid to_non_device :: El mismo JID sin dispositivo.
@fn Jid agent :: El código de tipo de servidor.
@fn Jid is_user :: Si es una cuenta.
@fn Jid is_pn :: Si usa número de teléfono.
@fn Jid is_lid :: Si usa LID.
@fn Jid is_group :: Si es un grupo.
@fn Jid is_broadcast :: Si es una lista de difusión o los estados.
@fn Jid is_status_broadcast :: Si son los estados.
@fn Jid is_newsletter :: Si es un canal.
@fn Jid is_server :: Si es un servidor, sin usuario.
@fn Jid same_user :: Si es la misma cuenta, ignorando el dispositivo.
@fn Jid signal_address :: La dirección que usa Signal internamente.
@fn Jid parse :: Lee un JID desde un texto.

## Grupos

@fn GroupInfo is_admin :: Si esa persona es administradora, buscando por número o por LID.
@fn GroupInfo participant :: Un participante, buscando por número o por LID.
@fn GroupInfo admins :: Los administradores.
@fn GroupInfo is_lid_addressed :: Si el grupo usa LID.

## Proxy

@fn Proxy parse :: Lee una URL de [proxy](#proxy).
@fn Proxy kind :: `Http` o `Socks5`.
@fn Proxy host :: El host.
@fn Proxy port :: El puerto.

## Tiempos

@fn SendTimings total :: La suma de todas las fases.
@fn crate::client::Stats to_json :: Las métricas en JSON.

## Privacidad

@fn PrivacySetting as_str :: El nombre del ajuste en el protocolo.
@fn PrivacyValue as_str :: El nombre del valor en el protocolo.

=== ref-store | Almacenamiento | Referencia de la API
lead: Los constructores de cada [store](#almacenamiento) y las funciones de migración.

## Stores

@fn FileStore open :: Abre o crea una carpeta de sesión.
@fn FileStore path :: La ruta de esa carpeta.
@fn MemoryStore new :: Un store en memoria.
@fn BatchedStore<Sqlite> open :: Abre o crea un archivo SQLite, para una cuenta.
@fn BatchedStore<Sqlite> open_account :: Lo mismo, para esa cuenta. Varias cuentas pueden compartir el archivo.
@fn BatchedStore<Sqlite> in_memory :: Una base SQLite en memoria.
@fn BatchedStore<Sqlite> from_connection :: Usa una conexión de `rusqlite` que ya tengas.
@fn BatchedStore<Postgres> connect :: Se conecta a PostgreSQL y crea las tablas si faltan.
@fn BatchedStore<Postgres> connect_account :: Lo mismo, para esa cuenta.
@fn BatchedStore<Postgres> from_client :: Usa un cliente de `tokio-postgres` que ya tengas.
@fn BatchedStore<Redis> connect :: Se conecta a Redis.
@fn BatchedStore<Redis> connect_account :: Lo mismo, para esa cuenta.
@fn BatchedStore<Mongo> connect :: Se conecta a MongoDB, a esa base de datos.
@fn BatchedStore<Mongo> connect_account :: Lo mismo, para esa cuenta.
@fn BatchedStore<Mongo> from_database :: Usa una base de datos del driver de MongoDB que ya tengas.
@fn BatchedStore<B> new :: Envuelve tu `Backend`, con caché y escrituras agrupadas.
@fn BatchedStore<B> backend :: Acceso a tu `Backend`.

## Traits

| Trait | Métodos obligatorios | Opcionales |
|---|---|---|
| `Store` | `load_creds`, `save_creds`, `get`, `set`, `remove`, `clear` | `flush`, `keys` |
| `Backend` | `load_creds`, `save_creds`, `fetch`, `apply` | `list` |

`KeyWrite` puede ser `Set(kind, key, value)`, `Remove(kind, key)` o `Clear`.

## Migración

@m import_baileys :: soblendwa::store::import_baileys(dir, store) -> Result<BaileysImport> :: Importa una carpeta de `useMultiFileAuthState`.
@m creds_from_baileys :: soblendwa::store::creds_from_baileys(value) -> Result<Creds> :: Convierte solo las credenciales de Baileys.
@m import_zapo_sqlite :: soblendwa::store::import_zapo_sqlite(path, session, store) -> Result<ZapoImport> :: Importa una sesión de zapo desde SQLite. Solo lee la base. Feature `sqlite`.
@m export_zapo_sqlite :: soblendwa::store::export_zapo_sqlite(store, path, session) -> Result<ZapoExport> :: Escribe el estado de vuelta en la base de zapo. Feature `sqlite`.
@m zapo_sessions :: soblendwa::store::zapo_sessions(path) -> Result<Vec<String>> :: Las sesiones que hay en una base de zapo. Feature `sqlite`.
@m ZapoMongo::connect :: ZapoMongo::connect(url, database) -> Result<ZapoMongo> :: Se conecta a una base de `@zapo-js/store-mongo`. Features `sqlite` y `mongo`.
@m ZapoMongo::new :: ZapoMongo::new(database) -> ZapoMongo :: Usa una base de datos del driver que ya tengas.
@m ZapoMongo::collection_prefix :: zapo_mongo.collection_prefix(prefix) -> ZapoMongo :: Indica el prefijo de colecciones.
@m ZapoMongo::sessions :: zapo_mongo.sessions() -> Result<Vec<String>> :: Las sesiones que hay en esa base.
@m import_zapo_mongo :: soblendwa::store::import_zapo_mongo(source, session, store) -> Result<ZapoImport> :: Importa una sesión. Solo lee MongoDB.
@m export_zapo_mongo :: soblendwa::store::export_zapo_mongo(store, target, session) -> Result<ZapoExport> :: Escribe el estado de vuelta en MongoDB.

=== ref-testing | FakeWhatsApp | Referencia de la API
lead: El [WhatsApp falso](#pruebas-falso), con el protocolo real. Requiere la feature `testing`.

@fn FakeWhatsApp start :: Arranca el servidor en un puerto libre.
@fn FakeWhatsApp connect :: Crea y conecta una cuenta. Devuelve su cliente y sus eventos.
@fn FakeWhatsApp connect_with :: Conecta una cuenta con un `ClientBuilder` propio.
@fn FakeWhatsApp builder :: Un `ClientBuilder` ya apuntando a este servidor.
@fn FakeWhatsApp account :: Credenciales vinculadas para ese número.
@fn FakeWhatsApp jid :: El JID de un número.
@fn FakeWhatsApp url :: La URL del WebSocket.
@fn FakeWhatsApp trust_root :: Su clave raíz, para `.trust_root(...)`.
@fn FakeWhatsApp media_endpoint :: Su servidor de multimedia.
@fn FakeWhatsApp media_uploads :: Cuántas subidas recibió.
@fn FakeWhatsApp expire_media :: Caduca los archivos subidos.
@fn FakeWhatsApp slow_uploads :: Hace que cada subida tarde eso, como un CDN lento.
@fn Client simulate_outage :: Corta el socket como una red caída, sin avisar al servidor; la reconexión y el outbox siguen su curso normal. Feature `testing`.
@fn Client damage_sessions :: Daña nuestro lado de la sesión Signal con un dispositivo, para probar el reintento con WhatsApp de verdad. Feature `testing`.
@fn FakeWhatsApp create_group :: Crea un grupo. Devuelve su JID.
@fn FakeWhatsApp drop_connections :: Corta la conexión de esa cuenta.
@fn FakeWhatsApp refuse_connections :: Rechaza o admite sus conexiones.
@fn FakeWhatsApp block :: Bloquea la cuenta con ese código.
@fn FakeWhatsApp unblock :: La desbloquea.
@fn FakeWhatsApp hold_acks :: Retiene o libera sus confirmaciones.
@fn FakeWhatsApp push_node :: Le envía un nodo cualquiera.
@fn FakeWhatsApp corrupt_next :: Daña sus siguientes mensajes, para probar reintentos.
@fn FakeWhatsApp routed :: Cuántos mensajes enrutó.
@fn FakeWhatsApp is_online :: Si esa cuenta está conectada.
@fn FakeWhatsApp add_to_group :: Añade gente a un grupo, como si lo hiciera ese administrador, y avisa a los miembros.
@fn FakeWhatsApp remove_from_group :: Saca gente de un grupo y avisa a los miembros y a quien salió.
@fn FakeWhatsApp invite_code :: El código de invitación de un grupo.
@fn FakeWhatsApp set_join_approval :: Activa o quita la aprobación para entrar.

=== ref-bajo-nivel | Bajo nivel | Referencia de la API
lead: Módulos públicos para casos avanzados: nodos binarios, criptografía e imágenes.

## Nodos binarios

`soblendwa::binary` contiene el formato con el que WhatsApp envía todo. Úsalo con [`client.query`](#api-client-query) y [`client.send_node`](#api-client-send-node).

```rust
use soblendwa::binary::Node;

let request = Node::new("iq")
    .attr("to", Jid::server())
    .attr("type", "get")
    .attr("xmlns", "w:p")
    .child_node(Node::new("ping"));
let respuesta = client.query(request).await?;
```

@fn Node new :: Un nodo con esa etiqueta.
@fn Node attr :: Añade un atributo.
@fn Node attr_opt :: Añade un atributo solo si hay valor.
@fn Node set_attr :: Añade un atributo a un nodo existente.
@fn Node children :: Pone varios hijos.
@fn Node child_node :: Añade un hijo.
@fn Node push_child :: Añade un hijo a un nodo existente.
@fn Node bytes :: Pone contenido binario.
@fn Node text :: Pone contenido de texto.
@fn Node get :: Lee un atributo.
@fn Node has :: Si tiene ese atributo.
@fn Node attr_str :: Lee un atributo como texto.
@fn Node attr_string :: Lee un atributo como `String`.
@fn Node attr_jid :: Lee un atributo como JID.
@fn Node attr_u64 :: Lee un atributo como número sin signo.
@fn Node attr_i64 :: Lee un atributo como número con signo.
@fn Node attr_is :: Si el atributo tiene ese valor.
@fn Node nodes :: Los hijos.
@fn Node into_nodes :: Los hijos, consumiendo el nodo.
@fn Node child :: El primer hijo con esa etiqueta.
@fn Node children_by :: Los hijos con esa etiqueta.
@fn Node first_child :: El primer hijo.
@fn Node path :: Baja por varias etiquetas.
@fn Node content_bytes :: El contenido binario.
@fn Node content_string :: El contenido como texto.
@fn Node child_bytes :: El contenido binario de un hijo.
@fn Node child_uint :: El contenido de un hijo como número.
@fn Node error_code :: El código y el texto de un nodo de error.
@fn Value as_str :: El valor como texto.
@fn Value to_jid :: El valor como JID.
@fn crate encode :: Codifica un nodo a bytes.
@fn crate decode :: Decodifica bytes a un nodo.
@fn crate decode_frame :: Decodifica un frame, descomprimiéndolo si hace falta.

## JSON

@fn Event kind :: El nombre del evento, como `message` o `pollVote`.
@fn Event to_json :: El evento en JSON, igual que en los demás lenguajes.
@fn Event to_json_string :: El mismo JSON ya como texto. Para mensajes, recibos, confirmaciones y presencia se escribe directo, unas seis veces más rápido que `to_json().to_string()`.
@fn crate node_to_json :: Un nodo en JSON.

## Credenciales

@fn Creds generate :: Genera credenciales nuevas, sin vincular.
@fn Creds is_logged_in :: Si tienen una cuenta vinculada.
@fn SignedPreKey generate :: Genera y firma una signed pre-key.
@fn KeyKind as_str :: El nombre del tipo de dato.

## Criptografía

`soblendwa::crypto` expone las piezas que usa la librería. Normalmente no las necesitas.

@fn crate sha256 :: SHA-256.
@fn crate sha256_concat :: SHA-256 de varias partes.
@fn crate hmac_sha256 :: HMAC-SHA256.
@fn crate hmac_sha512 :: HMAC-SHA512.
@fn crate hkdf :: HKDF-SHA256.
@fn crate hkdf_array :: HKDF-SHA256 hacia un arreglo.
@fn crate gcm :: Un cifrador AES-256-GCM.
@fn crate gcm_iv :: El IV que usa Noise, a partir de un contador.
@fn crate aes_gcm_encrypt :: Cifra con AES-GCM.
@fn crate aes_gcm_decrypt :: Descifra con AES-GCM.
@fn crate aes_cbc_encrypt :: Cifra con AES-CBC.
@fn crate aes_cbc_decrypt :: Descifra con AES-CBC.
@fn crate aes_ctr :: Cifra o descifra con AES-CTR.
@fn crate pbkdf2_sha256 :: PBKDF2 con SHA-256.
@fn crate md5 :: MD5, solo porque WhatsApp lo pide en un dato de registro.
@fn crate random_array :: Bytes aleatorios en un arreglo.
@fn crate random_vec :: Bytes aleatorios.
@fn crate random_u32 :: Un número aleatorio.
@fn crate constant_time_eq :: Compara en tiempo constante.
@fn KeyPair generate :: Un par de claves X25519 nuevo.
@fn KeyPair from_private :: Un par a partir de su clave privada.
@fn KeyPair shared_secret :: El secreto compartido con otra clave pública.
@fn KeyPair signal_public :: La clave pública con el prefijo de Signal.
@fn KeyPair sign :: Firma con XEdDSA.
@m crypto::sign :: soblendwa::crypto::sign(private, message) -> [u8; 64] :: Firma con XEdDSA a partir de una clave privada.
@m crypto::edwards_public :: soblendwa::crypto::edwards_public(private) -> [u8; 32] :: La clave pública de Edwards que usa la firma. Calcúlala una vez si vas a firmar muchas veces con la misma clave.
@m crypto::sign_with :: soblendwa::crypto::sign_with(private, edwards, message) -> [u8; 64] :: Igual que `sign`, con la clave de Edwards ya calculada.
@m crypto::verify :: soblendwa::crypto::verify(public, message, signature) -> bool :: Verifica una firma XEdDSA.
@m crypto::signal_key :: soblendwa::crypto::signal_key(public) -> [u8; 33] :: Añade el prefijo de Signal a una clave pública.
@m crypto::strip_key_prefix :: soblendwa::crypto::strip_key_prefix(key) -> Result<[u8; 32]> :: Quita el prefijo de Signal.

## Utilidades de la raíz

@m credits :: soblendwa::credits() -> String :: Nombre, versión y autor.
@m NAME :: soblendwa::NAME, VERSION, AUTHOR :: Constantes con esos datos.
@m fetch_latest_version :: soblendwa::fetch_latest_version(http) -> Result<[u32; 3]> :: Consulta la versión más reciente de WhatsApp Web.
@m DEFAULT_VERSION :: soblendwa::DEFAULT_VERSION :: La versión de respaldo.
@m COLLECTIONS :: soblendwa::COLLECTIONS :: Las colecciones de app state.
@m guess_mimetype :: soblendwa::guess_mimetype(path) -> Option<&str> :: El tipo MIME según la extensión.
@m generate_message_id :: soblendwa::message::generate_message_id(me) -> String :: Genera un id de mensaje.
@m media_json :: soblendwa::json::media_json(message) -> Value :: La multimedia de un mensaje en JSON.
@m image_dimensions :: soblendwa::imaging::image_dimensions(data) -> Option<(u32, u32)> :: El ancho y el alto de una imagen.
@m thumbnail :: soblendwa::imaging::thumbnail(data, width) -> Result<Vec<u8>> :: Una miniatura JPEG.
@m profile_picture :: soblendwa::imaging::profile_picture(data) -> Result<Vec<u8>> :: La imagen recortada al centro y en 640×640.

=== ref-tipos | Tipos | Referencia de la API
lead: Los tipos públicos que devuelven y reciben los métodos, y sus campos.

## Mensajes

| Tipo | Campos |
|---|---|
| `MessageEvent` | `info`, `message`, `raw` |
| `MessageInfo` | `id`, `chat`, `sender`, `sender_alt`, `sender_phone`, `sender_lid`, `from_me`, `is_group`, `push_name`, `timestamp`, `category`, `offline`, `edit`, `server_id`, `received_at` |
| `MessageKey` | `remote_jid`, `from_me`, `id`, `participant` |
| `SendOptions` | `id`, `edit`, `status_recipients`, `link_preview` |
| `PendingSend` | `id`, `chat`, `queued_at`, `attempts` |
| `InteractiveReply` | `id`, `text`, `source`, `flow`, `params` |
| `ReplySource` | `Buttons`, `List`, `Template`, `NativeFlow` |
| `NativeButton` | `QuickReply`, `Url`, `Copy`, `Call`, `Select` |
| `TemplateButton` | `QuickReply`, `Url`, `Call` |
| `Section`, `Row` | `title`, `rows`; `id`, `title`, `description` |

## Multimedia

| Tipo | Campos o valores |
|---|---|
| `MediaKind` | `Image`, `Video`, `Audio`, `Document`, `Sticker`, `History`, `AppState`, `LinkThumbnail` |
| `MediaOptions` | `mimetype`, `caption`, `file_name`, `voice_note`, `gif`, `view_once`, `seconds`, `width`, `height`, `thumbnail` |
| `UploadedMedia` | `url`, `direct_path`, `media_key`, `file_sha256`, `file_enc_sha256`, `file_length`, `reused` |
| `Downloadable` | Trait con `media_kind`, `media_key`, `direct_path`, `url`, `file_enc_sha256` y `file_sha256` |
| `LinkPreview` | `url`, `title`, `description`, `image_url`, `thumbnail`, `image`, `width`, `height`, `video` |
| `LinkPreviewConfig` | `enabled`, `timeout`, `upload_hq_thumbnail`, `allow_private_hosts`, `max_html_bytes`, `max_image_bytes`, `user_agent`, `cache_ttl`, `fetcher` |
| `PreviewChoice` | `Auto`, `On`, `Off`, `Custom(Box<LinkPreview>)` |
| `LinkThumbnail` | `direct_path`, `media_key`, `file_sha256`, `file_enc_sha256`, `width`, `height` |

## Grupos, canales y cuentas

| Tipo | Campos o valores |
|---|---|
| `GroupInfo` | `jid`, `subject`, `subject_owner`, `subject_time`, `owner`, `creation`, `description`, `description_id`, `announce`, `locked`, `ephemeral`, `addressing_mode`, `join_approval`, `members_can_add`, `linked_parent`, `is_community`, `is_community_announce`, `participants` |
| `GroupParticipant` | `jid`, `phone_number`, `lid`, `is_admin`, `is_super_admin` |
| `ParticipantAction` | `Add`, `Remove`, `Promote`, `Demote` |
| `ParticipantResult` | `jid`, `status` |
| `JoinRequest` | `jid`, `method`, `requested_at` |
| `LinkedGroup` | `jid`, `subject`, `owner`, `created`, `size` |
| `NewsletterInfo` | `jid`, `name`, `description`, `invite`, `subscribers`, `verified`, `picture_path`, `muted`, `role`, `created`, `raw` |
| `NewsletterMessage` | `server_id`, `id`, `timestamp`, `views`, `message`, `reactions` |
| `OnWhatsApp` | `query`, `jid`, `exists`, `lid` |
| `About` | `jid`, `text`, `set_at` |
| `BotInfo` | `jid`, `persona_id` |
| `DisappearingMode` | `jid`, `duration`, `set_at` |
| `ReachoutTimelock` | `active`, `ends_at`, `enforcement` |
| `ChatPresence` | `Composing`, `Recording`, `Paused` |
| `PrivacySetting` | `LastSeen`, `Online`, `ProfilePicture`, `About`, `ReadReceipts`, `GroupsAdd`, `Calls`, `Messages` |
| `PrivacyValue` | `Everyone`, `Contacts`, `ContactsExcept`, `Nobody`, `MatchLastSeen`, `Known` |
| `ChatModification` | `Archive`, `Pin`, `Mute`, `MarkRead`, `Delete`, `Clear`, `Star`, `DeleteForMe`, `SaveContact`, `RemoveContact`, `Label`, `MessageLabel` |

## Configuración

| Tipo | Campos o valores |
|---|---|
| `Config` | `device_name`, `platform`, `version`, `push_name`, `country_code`, `online_on_connect`, `auto_reconnect`, `connect_timeout`, `query_timeout`, `keepalive_interval`, `workers`, `ws_url`, `proxy`, `media_cache_ttl`, `send_limit`, `outbox`, `trust_root`, `media_endpoint`, `link_previews` |
| `Platform` | `Chrome`, `Firefox`, `Safari`, `Edge`, `Opera`, `Desktop` |
| `SendLimit` | `per_minute`, `burst`, `per_chat_per_minute`, `per_chat_burst` |
| `SendTimings` | `queued`, `upload`, `prepare`, `encrypt`, `server` |
| `TimingSummary` | `samples`, `queued`, `upload`, `prepare`, `encrypt`, `server`, `total` |
| `Spread` | `p50`, `p95`, `max` |
| `LogoutCause` | `RemovedFromPhone`, `SessionInvalidated`, `UserLogout` |
| `JoinVia` | `Created`, `Added`, `Invite` |
| `Stats` | [Ver Métricas](#confiabilidad-metricas) |
| `ProxyKind` | `Http`, `Socks5` |
| `EventStream` | El receptor de eventos: `recv().await` |

## Almacenamiento

| Tipo | Campos |
|---|---|
| `Creds` | `noise_key`, `pairing_ephemeral_key`, `identity_key`, `signed_pre_key`, `registration_id`, `adv_secret_key`, `next_pre_key_id`, `first_unuploaded_pre_key_id`, `me`, `lid`, `push_name`, `account`, `platform`, `business_name`, `pairing_code`, `routing_info`, `registered`, `wa_version`, `app_state_key_id`, `signed_pre_key_rotated_at` |
| `BaileysImport` | `me`, `sessions`, `pre_keys`, `sender_keys`, `app_state_keys`, `app_state_versions`, `lid_mappings`, `skipped`, `failed` |
| `ZapoImport` | `me`, `sessions`, `pre_keys`, `sender_keys`, `app_state_keys`, `app_state_versions`, `lid_mappings`, `failed` |
| `ZapoExport` | `sessions`, `pre_keys`, `sender_keys`, `app_state_keys`, `app_state_versions`, `failed` |

Los tipos de chats y de empresa están en [Store de chats](#store-chats) y en [Catálogo de empresa](#empresa).

=== ref-eventos | Eventos | Referencia de la API
lead: Todo lo que puede llegar por el canal de eventos, en Rust y en JSON.

En Node.js, Python y el servidor, cada evento es un objeto con un campo `type`, que es el nombre de esta tabla, y los demás campos en camelCase.

## Conexión

| Rust | `type` | Campos en JSON | Cuándo ocurre |
|---|---|---|---|
| `Qr(String)` | `qr` | `code` | Hay un QR nuevo para escanear |
| `PairSuccess { .. }` | `pairSuccess` | `id`, `lid`, `platform`, `businessName` | El teléfono aprobó la vinculación |
| `Connected` | `connected` | — | La sesión está lista |
| `Disconnected { .. }` | `disconnected` | `reason: { type, detail?, code? }`, `reconnecting` | Se perdió la conexión. Ver [motivos](#vincular-motivos-de-desconexion) |
| `LoggedOut { reason, cause, code }` | `loggedOut` | `reason`, `cause`, `code` | Se cerró la sesión y se borraron los datos. Ver [causas](#vincular-causa) |
| `AccountBlocked { .. }` | `accountBlocked` | `code`, `reason`, `expires` | WhatsApp bloqueó la cuenta. No reintenta y conserva la sesión |
| `OfflineSyncComplete { count }` | `offlineSyncComplete` | `count` | Ya llegaron los mensajes pendientes |

## Mensajes

| Rust | `type` | Campos en JSON | Cuándo ocurre |
|---|---|---|---|
| `Message(Box<MessageEvent>)` | `message` | Ver abajo | Llegó un mensaje descifrado |
| `UndecryptableMessage { .. }` | `undecryptableMessage` | `info`, `error` | No se pudo descifrar y ya se pidió un reintento |
| `MessageRecovered { .. }` | `messageRecovered` | `id`, `chat`, `sender` | Uno de esos llegó bien tras el reintento |
| `MessageLost { .. }` | `messageLost` | `id`, `chat`, `sender`, `error`, `waitedSecs` | No llegó dentro del plazo. Si llega después, igual se recupera |
| `Receipt(Receipt)` | `receipt` | `chat`, `sender`, `ids`, `receipt`, `timestamp`, `fromMe` | Entregado, leído, reproducido o reintento |
| `ServerAck(ServerAck)` | `serverAck` | `id`, `chat`, `error`, `latencyMicros`, `timings` | WhatsApp confirmó o rechazó un envío |
| `PollVote(PollVote)` | `pollVote` | `chat`, `pollId`, `voter`, `selected`, `timestamp` | Alguien votó |
| `EventResponse(EventResponse)` | `eventResponse` | `chat`, `eventId`, `responder`, `response`, `extraGuests`, `timestamp` | Alguien respondió a un evento |
| `HistorySync(Box<HistorySync>)` | `historySync` | `syncType`, `progress`, `chunkOrder`, `conversations`, `pushNames`, `lidMappings` | Llegó un bloque del historial |

### El mensaje en JSON

```json
{
  "type": "message",
  "info": {
    "id": "3EB0ABC123",
    "chat": "5215512345678@s.whatsapp.net",
    "sender": "5215512345678@s.whatsapp.net",
    "senderAlt": null,
    "senderPhone": "5215512345678@s.whatsapp.net",
    "senderLid": "100000000000001@lid",
    "fromMe": false,
    "isGroup": false,
    "pushName": "Ana",
    "timestamp": 1793729773,
    "category": null,
    "offline": false,
    "edit": null,
    "serverId": null,
    "elapsedMicros": 42
  },
  "kind": "text",
  "mediaType": null,
  "text": "hola",
  "media": null,
  "quoted": null,
  "viewOnce": false,
  "linkPreview": null,
  "ageSecs": 0,
  "quotedId": null,
  "mentions": [],
  "reaction": null,
  "reply": null,
  "proto": "CgRob2xh"
}
```

`media.type` puede ser `image`, `video`, `gif`, `ptv`, `audio`, `ptt`, `document` o `sticker`.

### Tipos de recibo

| Rust | En JSON | Qué significa |
|---|---|---|
| `Delivered` | `delivered` | Llegó al dispositivo |
| `Sender` | `sender` | Llegó a otro dispositivo tuyo |
| `Read`, `ReadSelf` | `read`, `read-self` | Lo leyeron. `ReadSelf`, tú desde otro dispositivo |
| `Played`, `PlayedSelf` | `played`, `played-self` | Lo reprodujeron |
| `Retry` | `retry` | No lo pudieron descifrar y piden otro envío. Se atiende solo |
| `Inactive` | `inactive` | El dispositivo estaba inactivo |
| `PeerMessage` | `peer_msg` | Lo recibió otro dispositivo tuyo |
| `HistorySync` | `hist_sync` | Recibo de historial |
| `ServerError` | `server-error` | El servidor no pudo entregarlo |
| `Other(texto)` | ese texto | Otro tipo |

## Personas, grupos y chats

| Rust | `type` | Campos en JSON | Cuándo ocurre |
|---|---|---|---|
| `Presence(Presence)` | `presence` | `from`, `available`, `lastSeen` | Alguien se conectó o desconectó |
| `ChatState(ChatState)` | `chatState` | `chat`, `sender`, `state` | Alguien escribe o graba |
| `GroupUpdate(GroupUpdate)` | `groupUpdate` | `group`, `author`, `action`, `participants`, `timestamp` | [Cambió un grupo](#grupos-cambios-en-vivo) |
| `JoinedGroup { .. }` | `joinedGroup` | `group`, `author`, `via` | El bot [entró a un grupo](#grupos-entrar-salir), de cualquier forma |
| `LeftGroup { .. }` | `leftGroup` | `group`, `author`, `removed` | El bot salió o lo sacaron |
| `LidMapping { phone, lid }` | `lidMapping` | `phone`, `lid` | Se descubrió que un número y un LID son la misma persona |
| `ChatUpdate { .. }` | `chatUpdate` | `update`, `timestamp`, `initialSync` | [Cambió un chat o contacto](#chats-cambios-desde-el-telefono) |
| `PictureChanged { .. }` | `pictureChanged` | `jid`, `author`, `pictureId`, `removed`, `timestamp` | Alguien cambió o quitó su foto {exp} |
| `AboutChanged { .. }` | `aboutChanged` | `jid`, `about`, `timestamp` | Alguien cambió su información {exp} |
| `BlocklistChanged { items }` | `blocklistChanged` | `items: [{ jid, phone, blocked }]` | Cambió tu lista de bloqueados {exp} |
| `PrivacyChanged { settings }` | `privacyChanged` | `settings: [{ name, value }]` | Cambió un ajuste de privacidad {exp} |
| `Call(CallEvent)` | `call` | `id`, `from`, `call`, `video`, `isGroup`, `timestamp` | Llamada entrante, aceptada, rechazada o terminada |

## Avanzado

| Rust | `type` | Campos en JSON | Cuándo ocurre |
|---|---|---|---|
| `Notification(Node)` | `notification` | `node: { tag, attrs, content }` | Una notificación sin tipo propio |
| `Unhandled(Node)` | `unhandled` | `node: { tag, attrs, content }` | Un nodo que la librería no atiende |

=== ref-errores | Errores | Referencia de la API
lead: Los errores que puede devolver cualquier método, y qué hacer con cada uno.

`soblendwa::Result<T>` usa `soblendwa::Error`. Cada error tiene un código estable, `error.code()` en Rust y `error.code` en los demás lenguajes. Compara siempre el código, nunca el texto. Si el error viene de WhatsApp, `status` trae su número.

En Node.js y el cliente JS llegan como `SoblendError`, en Python como `SoblendError`, y por HTTP y WebSocket con `code` y `status` en el JSON.

## Códigos que más vas a usar

| Código | Cuándo aparece | Qué hacer |
|---|---|---|
| `APPROVAL_REQUIRED` | Entraste con un enlace a un grupo que pide aprobación | La solicitud ya se envió. Espera `joinedGroup` |
| `PRIVATE_PHOTO` | La foto de perfil existe pero no te dejan verla | Avísale al usuario que es privada |
| `FORBIDDEN` | WhatsApp no te deja hacerlo. En grupos, casi siempre porque el bot no es administrador | Pide que lo hagan admin |
| `NOT_FOUND` | No existe: un grupo, un enlace de invitación o un usuario | Revisa el dato |
| `GONE` | Ya no existe, por ejemplo un enlace revocado | Pide uno nuevo |
| `NOT_ACCEPTABLE` | WhatsApp rechazó la petición, por ejemplo una invitación que no es válida para ti | Revisa el dato |
| `RATE_OVERLIMIT` | WhatsApp está limitando tus peticiones | Espera antes de reintentar |
| `RATE_LIMITED` | Lo frenó el [límite de envíos](#confiabilidad-limite) de SoblendWA | Espera `retry_after` |
| `MESSAGE_NOT_FOUND` | En Node.js, Python o el servidor, el id ya no está entre los mensajes recientes | Guarda el mensaje o activa el store de chats |
| `NO_DEVICES` | El número no tiene ningún dispositivo con WhatsApp | Revisa el número con `onWhatsApp` |
| `INVALID_ARGUMENT` | Falta un parámetro o tiene un valor que no sirve | Lee el mensaje: dice cuál |
| `TIMEOUT` | No hubo respuesta a tiempo | Reintenta |
| `NOT_CONNECTED`, `NOT_LOGGED_IN` | La cuenta no está conectada o no está vinculada | Espera `connected` o vincúlala |

Los errores de WhatsApp sin código propio llegan como `BAD_REQUEST` (400), `NOT_AUTHORIZED` (401), `NOT_ALLOWED` (405), `REQUEST_TIMEOUT` (408), `CONFLICT` (409), `INTERNAL_SERVER_ERROR` (500), `NOT_IMPLEMENTED` (501), `SERVICE_UNAVAILABLE` (503) o `SERVER_ERROR` para los demás.

## Todos los errores de Rust

| Error | Código | Qué significa | Qué hacer |
|---|---|---|---|
| `WebSocket` | `CONNECTION_FAILED` | Falló el socket | Revisa la red. Se reintenta solo |
| `Io` | `IO_ERROR` | Falló el disco o la red | Revisa la ruta y los permisos |
| `Http` | `HTTP_ERROR` | Falló una petición HTTP | Revisa la red o el proxy |
| `Proto` | `PROTOBUF_ERROR` | Un protobuf no se pudo leer | Abre un issue si se repite |
| `Json` | `INVALID_JSON` | Un JSON no se pudo leer | Revisa los parámetros |
| `Binary` | `MALFORMED_NODE` | Un nodo está mal formado | Abre un issue si se repite |
| `Crypto` | `CRYPTO_ERROR` | Falló una operación de cifrado | Puede ser un archivo dañado |
| `Signal` | `SIGNAL_ERROR` | Falló el protocolo Signal | Normalmente se recupera con un reintento |
| `Handshake` | `HANDSHAKE_FAILED` | Falló el handshake con el servidor | Revisa la red, el proxy o `trust_root` |
| `Server { code, text }` | Según `code`, ver arriba | WhatsApp rechazó la petición | El código indica el motivo |
| `Timeout` | `TIMEOUT` | No hubo respuesta a tiempo | Reintenta. `error.is_timeout()` lo detecta |
| `NotConnected` | `NOT_CONNECTED` | No hay conexión | Espera a `Connected` |
| `NotLoggedIn` | `NOT_LOGGED_IN` | La cuenta no está vinculada | Vincúlala |
| `InvalidJid` | `INVALID_JID` | El JID no es válido | Revisa el texto |
| `Media` | `MEDIA_ERROR` | Falló la subida o descarga | Revisa el archivo |
| `Store` | `STORE_ERROR` | Falló el almacenamiento | Revisa tu base de datos |
| `MissingAppStateKey` | `MISSING_APP_STATE_KEY` | Falta una clave de app state | Se pide sola al teléfono |
| `RateLimited { chat, retry_after }` | `RATE_LIMITED` | Pasaste el [límite](#confiabilidad-limite) | Espera `retry_after` |
| `AccountBlocked` | `ACCOUNT_BLOCKED` | [Cuenta bloqueada](#confiabilidad-bloqueo) | Revísala en el teléfono |
| `ApprovalRequired(grupo)` | `APPROVAL_REQUIRED` | El grupo pide aprobación y la solicitud ya se envió | Espera `JoinedGroup` |
| `PrivatePhoto(jid)` | `PRIVATE_PHOTO` | La foto de perfil es privada | Avísale al usuario |
| `MessageNotFound(id)` | `MESSAGE_NOT_FOUND` | El mensaje ya no está entre los recientes | Guárdalo o usa el store de chats |
| `NoDevices(jid)` | `NO_DEVICES` | El número no tiene dispositivos con WhatsApp | Revisa el número con `on_whatsapp` |
| `InvalidArgument(texto)` | `INVALID_ARGUMENT` | Un parámetro falta o no sirve | Lee el mensaje |
| `Other` | `OTHER` | Otro error | Lee el mensaje |

@fn Error is_timeout :: Indica si el error es `Timeout`.
@fn Error code :: El código estable del error, como `APPROVAL_REQUIRED` o `FORBIDDEN`.
@fn Error status :: El número de error de WhatsApp, si vino de WhatsApp.
