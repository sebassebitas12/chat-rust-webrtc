# Chat Rust WebRTC

Aplicación web privada 1:1 para que dos personas creen una cuenta, se encuentren por ID, acepten el contacto, conversen y hagan llamadas de voz.

## Stack

- **Frontend:** React 19, TypeScript y Vite 8 en `frontend/`.
- **Backend:** Rust, Axum y Tokio en `backend/`.
- **Persistencia:** MySQL mediante SQLx y migraciones versionadas.
- **Tiempo real:** WebSocket para presencia, mensajes y señalización; WebRTC para audio.
- **Despliegue:** Docker multietapa en Railway; Axum sirve la app React y la API desde el mismo dominio.

## Funciones incluidas

- Registro e inicio/cierre de sesión con cookie `HttpOnly`; contraseñas con hash Argon2.
- ID público único para agregar personas sin mostrar correos.
- Solicitudes de contacto con aceptación o rechazo.
- Mensajes privados persistentes, historial reciente, presencia y vista previa.
- Foto opcional de perfil: JPG, PNG o WebP de hasta 5 MB; se valida, reduce y almacena en MySQL.
- Llamadas de audio 1:1, permiso de micrófono, aceptación/rechazo, silencio y colgar.

## Estado de entrega

El frontend de producción compila localmente. El backend, el despliegue actualizado y una conversación/llamada entre dos cuentas deben quedar verificados en CI y Railway antes de presentar el enlace como listo. Consultá [`docs/DEPLOY.md`](docs/DEPLOY.md) para completar las variables y hacer la prueba de dos usuarios.

## Puesta en marcha en Railway

El repositorio usa el `Dockerfile` de la raíz y requiere una base MySQL conectada mediante `DATABASE_URL`. Al iniciar, el backend aplica las migraciones. Railway provee `RAILWAY_PUBLIC_DOMAIN`; el servidor lo usa para validar el origen y activar cookies seguras automáticamente. Si cambiás a un dominio personalizado, configurá `APP_BASE_URL=https://tu-dominio`.

Para que las llamadas funcionen con más tipos de NAT y redes, configurá los secretos TURN opcionales indicados en [`docs/DEPLOY.md`](docs/DEPLOY.md). Sin ellos se usa STUN y algunas combinaciones de redes no lograrán conectar audio.

## Desarrollo local

1. Instalá Rust estable (1.89 o superior), Node.js 24 y MySQL 8.
2. Creá una base vacía y copiá `.env.example` a `.env`; completá `DATABASE_URL`.
3. En una terminal ejecutá `cargo run --manifest-path backend/Cargo.toml` desde la raíz.
4. En otra terminal ejecutá `npm ci` y `npm run dev` desde `frontend/`.

Vite reenvía `/api` y `/api/v1/realtime` a `http://127.0.0.1:8080`. El endpoint `/health` indica si el backend y MySQL están conectados.

## Carpetas

```text
frontend/       React, TypeScript, estilos y cliente de API
backend/        API Rust, WebSocket y migraciones MySQL
docs/           arquitectura e instrucciones de despliegue
mockups/        referencias visuales aprobadas
Dockerfile      construcción de frontend y backend para Railway
```

La llamada no se graba ni se almacena. Mensajes y cuentas sí se guardan en MySQL. Este MVP es un solo servicio y el estado de presencia/señalización vive en memoria; mantené una sola réplica del servicio web.