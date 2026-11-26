# Chat Rust WebRTC

Aplicación web de chat privado 1:1 con perfiles, contactos por ID único, mensajes y llamadas de voz en tiempo real.

## Stack

- **Frontend:** React 19, TypeScript y Vite 8.
- **Backend:** Rust, Axum y Tokio.
- **Datos:** MySQL local y MySQL en Railway.
- **Tiempo real:** WebSocket para presencia y señalización; WebRTC para audio.
- **Fotos:** almacenamiento local en desarrollo y Railway Bucket en producción.

## Estado

Primera base de frontend y backend en preparación. El formulario de cuenta es solo interfaz; registro, MySQL, contactos, chat y llamadas aún deben implementarse.

## Alcance inicial

- Registro, inicio de sesión y perfil con foto.
- ID público para encontrar personas.
- Solicitud de contacto que la otra persona puede aceptar o rechazar.
- Chat privado y llamada de voz 1:1 entre contactos.

El servicio web se construye desde el `Dockerfile` de la raíz. React y Rust siguen separados en `frontend/` y `backend/`; Axum sirve los archivos compilados de React y `/health` reporta el estado del servidor.
