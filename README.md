# Chat Rust WebRTC

Aplicación web de chat privado 1:1 con perfiles, contactos por ID único, mensajes y llamadas de voz en tiempo real.

## Stack previsto

- **Frontend:** React, TypeScript y Vite.
- **Backend:** Rust, Axum y Tokio.
- **Datos:** MySQL local y MySQL en Railway.
- **Tiempo real:** WebSocket para presencia y señalización; WebRTC para audio.
- **Archivos:** fotos de perfil locales durante el desarrollo y almacenamiento compatible con S3 en Railway.

## Estado

En planificación. El repositorio todavía no contiene el código de la aplicación. El nombre visual del producto también está pendiente.

## Alcance inicial

- Registro, inicio de sesión y perfil con foto.
- ID público para encontrar personas.
- Solicitud de contacto que la otra persona puede aceptar o rechazar.
- Chat privado y llamada de voz 1:1 entre contactos.

