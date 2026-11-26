# Despliegue y prueba entre dos personas

## Railway

1. Conectá `sebassebitas12/chat-rust-webrtc` a un servicio desde la rama `main`. El builder es el `Dockerfile` de la raíz.
2. Agregá el servicio MySQL al mismo proyecto y ambiente. En el servicio de la app, definí `DATABASE_URL` como referencia a `${{MySQL.MYSQL_URL}}` (usá exactamente el nombre de tu servicio si no es `MySQL`).
3. Aplicá y desplegá los cambios pendientes en Railway. El backend escucha en `PORT` y ejecuta las migraciones al arrancar.
4. Generá un dominio HTTP para el servicio web con destino al puerto `8080`. No expongás MySQL al público ni creés un TCP Proxy para la base.
5. Dejá una sola réplica del backend: los sockets, presencia y señales de llamada viven en memoria.
6. Abrí `https://tu-dominio/health`. Debe responder JSON con `status: "ok"` y `database: "connected"`.

Railway proporciona `RAILWAY_PUBLIC_DOMAIN`; la app lo usa si no configurás `APP_BASE_URL`. Si usás un dominio propio, establecé `APP_BASE_URL=https://tu-dominio` y `COOKIE_SECURE=true`. Después de cambiar variables, revisá los cambios y volvé a desplegar.

## TURN para llamadas entre redes distintas

La app usa STUN de Cloudflare cuando no hay credenciales TURN. STUN permite conexiones directas en muchas redes, pero no garantiza que los navegadores detrás de NAT restrictivo se alcancen. Para completar una llamada más confiable:

1. En Cloudflare, creá una cuenta de Realtime y una TURN key/API token con permiso para generar credenciales temporales.
2. En **Variables** del servicio web de Railway, agregá `CF_TURN_KEY_ID` y `CF_TURN_API_TOKEN`. Guardá el API token como secreto de Railway; no lo pegues en el repo ni en el chat.
3. Desplegá las variables y verificá llamadas desde dos redes distintas.

El backend usa esa API token para pedir credenciales de corta duración; el navegador solo recibe ICE servers y credenciales efímeras. Cloudflare Realtime TURN cobra por el tráfico retransmitido; revisá su tarifa y el método de pago antes de probar llamadas largas.

## Prueba funcional de dos usuarios

1. Vos y tu compañero abran el mismo enlace en Chrome, Edge o Firefox actualizado, con micrófono disponible y permiso HTTPS.
2. Cada uno crea su cuenta con un correo distinto y una contraseña de 12 a 128 caracteres. La foto es opcional.
3. Intercambien el ID público de ocho caracteres que aparece en el perfil; no compartan contraseñas, cookies ni tokens.
4. Una persona busca el ID de la otra y envía la solicitud. La persona receptora la acepta desde la lista.
5. Envíen mensajes en ambos sentidos, recarguen y comprueben que el historial sigue allí.
6. Con ambas sesiones abiertas, inicien una llamada, acepten en el otro dispositivo, prueben silencio y finalización.
7. Repitan con ambas personas en redes distintas. Si no conecta, revisen los logs del servicio y configuren Cloudflare TURN.

## Alcance de privacidad

Las contraseñas se almacenan con hash Argon2 y las sesiones usan cookies `HttpOnly`/`Secure`. Los mensajes y perfiles se guardan en MySQL y se protegen durante el transporte por HTTPS/WSS; no hay cifrado de extremo a extremo. El servicio que opera la aplicación y quienes tengan acceso a MySQL pueden acceder al contenido. Las llamadas de audio no se graban ni se almacenan.

## Variables

| Variable | Requerida | Uso |
| --- | --- | --- |
| `DATABASE_URL` | Sí | URL privada de MySQL en Railway. |
| `PORT` | Railway | Puerto HTTP asignado por Railway; valor local de ejemplo `8080`. |
| `APP_BASE_URL` | Solo dominio propio | Origen HTTPS usado para verificar solicitudes y configurar cookies; el dominio `up.railway.app` se detecta automáticamente. |
| `COOKIE_SECURE` | Solo si se fija manualmente | Usá `true` con HTTPS; por defecto se infiere del origen. |
| `CF_TURN_KEY_ID` | Opcional | Identificador de la TURN key de Cloudflare. |
| `CF_TURN_API_TOKEN` | Opcional y secreto | Token restringido para solicitar credenciales efímeras TURN. |

## Fallas comunes

- **`/health` indica error de base:** confirmá que `DATABASE_URL` apunta al `MYSQL_URL` del servicio MySQL en el mismo ambiente y redeplegá.
- **No permite registro o login:** comprobá en logs que las migraciones terminaron y que `APP_BASE_URL` coincide exactamente con el dominio, incluyendo `https://`.
- **No sale el permiso de micrófono:** abrí el dominio HTTPS y permití el micrófono para el sitio.
- **La llamada timbra y falla al conectar:** confirmá que ambos WebSockets están en línea y habilitá TURN; STUN solamente no alcanza todas las redes.
