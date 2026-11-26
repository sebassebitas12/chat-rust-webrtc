# Chat Rust WebRTC

Aplicación web de chat privado 1:1 con perfiles, contactos por ID único, mensajes y llamadas de voz en tiempo real.

## Stack

- **Frontend:** React 19, TypeScript y Vite 8.
- **Backend:** Rust, Axum y Tokio.
- **Datos:** MySQL local y MySQL en Railway.
- **Tiempo real:** WebSocket para presencia y señalización; WebRTC para audio.
- **Fotos:** almacenamiento local en desarrollo y Railway Bucket en producción.

## Estado

La pantalla inicial y el servidor Axum están desplegados. El registro ya crea una cuenta en MySQL, guarda la contraseña como hash Argon2 y devuelve el ID público. El inicio de sesión, las sesiones, el guardado de fotos, los contactos, el chat y las llamadas todavía están pendientes.

## Alcance inicial

- Registro, inicio de sesión y perfil con foto.
- ID público para encontrar personas.
- Solicitud de contacto que la otra persona puede aceptar o rechazar.
- Chat privado y llamada de voz 1:1 entre contactos.

El servicio web se construye desde el `Dockerfile` de la raíz. React y Rust siguen separados en `frontend/` y `backend/`; Axum sirve los archivos compilados de React y `/health` reporta el estado del servidor.

Para el desarrollo local, copiá `.env.example` a `.env` y configurá `DATABASE_URL` con tu MySQL. En Railway, agregá `DATABASE_URL` como referencia a la variable `MYSQL_URL` del servicio MySQL; no pegues credenciales en Git ni en el chat. Si no configurás la variable, el sitio inicia sin base y `/health` indica `not_configured`.

## Primer endpoint

`POST /api/v1/auth/register` recibe un JSON con `display_name`, `email` y `password`. Responde `201` con `public_id` y `display_name`; no devuelve el correo ni el hash. La selección de foto todavía es una vista previa: el archivo no se guarda en esta etapa.
