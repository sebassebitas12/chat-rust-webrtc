# Arquitectura propuesta — chat y llamadas 1 a 1

> Estado: planificación. Esta estructura define el proyecto; todavía no hay código de la aplicación ni conexión real de audio.

## Objetivo y flujo del MVP

Una persona crea una cuenta con nombre visible, correo y contraseña, y puede subir una foto de perfil. El sistema genera un **ID público único** para compartirlo con otras personas; el correo nunca se usa como identificador público ni se muestra en la búsqueda. Con ese ID se encuentra a otra cuenta y se envía una solicitud de contacto. Cuando la otra persona la acepta, pueden conversar por mensajes privados y llamarse por voz 1 a 1 si están conectadas.

La llamada entrante se puede aceptar o rechazar. Durante la llamada se puede silenciar el micrófono o colgar. La interfaz muestra claramente los estados de contacto y llamada (pendiente, conectado, ocupado, sin conexión, timbrando, conectada y finalizada).

El MVP no incluye grupos, video, archivos adjuntos ni historial de llamadas. Los mensajes y las relaciones de contacto sí se guardan. La llamada activa es efímera: no se graba audio ni se conserva su contenido; al reiniciar el servidor, se corta.

### Recorrido principal

1. Registro con nombre visible, correo y contraseña; la foto se puede subir durante el registro o después desde el perfil.
2. El servidor crea un ID público no secuencial, único e inmutable, por ejemplo `7K4P2D9Q`, independiente del nombre provisional de la app. La persona puede copiarlo y compartirlo.
3. En “Agregar persona”, se busca por ID exacto. El resultado muestra ID, nombre y foto, sin revelar correo.
4. Se envía una solicitud. El chat y las llamadas se habilitan después de que la otra persona la acepte; solicitudes pendientes se pueden aceptar o rechazar.
5. Desde el chat se envían mensajes y se inicia una llamada de voz. La otra persona recibe una invitación entrante y puede aceptar o rechazarla.

La foto de perfil admite JPEG, PNG o WebP con un límite inicial de 5 MB. El backend valida el contenido real y el tamaño, asigna una clave aleatoria al archivo y guarda solo esa clave en la base de datos; nunca confía en el nombre o el tipo MIME enviado por el navegador. SVG y archivos ejecutables no se aceptan como foto. La persona puede reemplazar o quitar su foto.

## Stack propuesto

### Frontend

- React + TypeScript + Vite para la aplicación web.
- CSS propio organizado por componentes y tokens visuales.
- `fetch` para las operaciones normales de la API.
- WebSocket para presencia e intercambio de señalización de llamadas.
- WebRTC del navegador para el audio en vivo.

### Backend

- Rust estable y Cargo.
- Axum para rutas HTTP y conexiones WebSocket.
- Tokio para el runtime asíncrono.
- Serde para convertir datos Rust a JSON y viceversa.
- SQLx + MySQL para cuentas, sesiones, contactos, conversaciones y mensajes, usando MySQL local en desarrollo y el servicio MySQL de Railway en despliegue.
- Argon2 para almacenar hashes de contraseñas, nunca contraseñas en texto plano.
- Almacenamiento de avatares detrás de una interfaz común: directorio local durante el desarrollo y Railway Bucket privado en producción.

## Cómo viaja una llamada

1. El backend comprueba que las dos personas registradas estén conectadas y transmite la invitación.
2. Al aceptar, los clientes intercambian por WebSocket la información de negociación WebRTC (oferta, respuesta y candidatos ICE).
3. WebRTC establece el canal de audio entre los dos navegadores. El backend coordina el inicio y la presencia; no guarda audio ni historial de llamadas.
4. STUN ayuda a encontrar una ruta directa. Algunas redes requieren TURN para retransmitir el tráfico; eso se comprobará al desplegar y probar en la red de clase.

## Datos que persiste MySQL

- `users`: ID interno, ID público único, nombre visible, correo único, hash de contraseña, clave del avatar opcional y fechas de creación/actualización.
- `sessions`: sesión autenticada, vencimiento y revocación; los secretos se almacenan como hash.
- `contact_requests`: emisor, destinatario, estado (`pending`, `accepted`, `rejected`) y fechas. La pareja de usuarios no puede tener solicitudes duplicadas ni autoenviarse solicitudes.
- `conversations`: identificador y fecha de creación; una conversación privada por pareja de contactos.
- `conversation_members`: relación entre conversación y sus dos usuarios.
- `messages`: conversación, remitente, texto y fechas de creación/edición. Un usuario solo puede leer o enviar mensajes dentro de una conversación a la que pertenece.

No habrá tabla `calls` en el MVP. Las llamadas son efímeras; si el proceso del backend se reinicia, la llamada activa se corta.

## Rutas y eventos previstos

- HTTP: registro e inicio/cierre de sesión; consulta/edición de perfil y carga/eliminación de avatar; búsqueda exacta por ID; envío, aceptación y rechazo de contactos; apertura de conversación y operaciones de mensajes.
- WebSocket autenticado: presencia y señalización de llamadas (`incoming`, `accept`, `decline`, `offer`, `answer`, `ice-candidate`, `end`).
- El backend valida la sesión y la relación de contacto/conversación en cada acción. Solo la pareja de contactos aceptada puede iniciar una conversación o una llamada. Se limita la frecuencia de intentos de inicio de sesión, búsqueda y solicitudes para dificultar abuso y enumeración de cuentas.
- Las cookies de sesión se configuran `HttpOnly`, `SameSite` y `Secure` en producción; el frontend no guarda credenciales ni tokens de sesión en `localStorage`.

## Estructura propuesta

```text
Sistema pequeño con IA/
├─ frontend/
│  ├─ public/
│  └─ src/
│     ├─ app/                 # montaje y rutas de React
│     ├─ features/
│     │  ├─ auth/             # crear cuenta e iniciar sesión
│     │  ├─ profile/          # nombre, ID público y foto de perfil
│     │  ├─ contacts/         # búsqueda por ID y solicitudes
│     │  ├─ chat/             # conversaciones y CRUD de mensajes
│     │  └─ calls/            # invitación, llamada y controles WebRTC
│     ├─ shared/
│     │  ├─ components/       # piezas visuales reutilizables
│     │  ├─ api/              # cliente HTTP
│     │  ├─ realtime/         # cliente WebSocket
│     │  └─ styles/           # tokens y estilos globales
│     ├─ main.tsx
│     └─ vite-env.d.ts
├─ backend/
│  ├─ migrations/             # esquema MySQL versionado
│  ├─ data/avatars/           # fotos locales de desarrollo, excluidas de Git
│  ├─ src/
│  │  ├─ main.rs              # arranque del servidor
│  │  ├─ config.rs            # configuración y variables de entorno
│  │  ├─ state.rs             # estado compartido de la aplicación
│  │  ├─ routes/              # auth, profile, contacts, conversations, messages, realtime
│  │  ├─ auth/                # contraseñas, sesión y middleware
│  │  ├─ services/            # reglas de cuenta, contactos, chat, presencia y llamadas
│  │  ├─ db.rs                # conexión al MySQL
│  │  └─ errors.rs            # errores HTTP consistentes
│  ├─ Cargo.toml
├─ docs/
│  ├─ ARQUITECTURA-PROPUESTA.md
│  ├─ INVESTIGACION-STACK.md
│  ├─ REGISTRO-DE-PROMPTS.md
│  ├─ REFLEXION.md
│  └─ DEPLOY.md
├─ mockups/
├─ Dockerfile                 # build del frontend y backend para Railway
├─ .dockerignore
├─ .env.example
├─ .gitignore
└─ README.md
```

## Repositorio y despliegue en Railway

- Se usará un repositorio GitHub propio para esta app, separado del laboratorio anterior. Será un monorepo con `frontend/`, `backend/`, `docs/` y `mockups/`; `main` será la rama que Railway despliega.
- La aplicación se desplegará inicialmente como un solo servicio web de Railway construido por el `Dockerfile` de la raíz. El frontend y el backend mantienen carpetas separadas, pero Axum sirve los archivos estáticos compilados de React. Así comparten dominio para sesión y WebSocket y evitamos duplicar servicios de aplicación durante el MVP.
- El proyecto Railway tendrá además un servicio MySQL y un Bucket privado para las fotos. El backend se conecta a MySQL por la red privada de Railway y recibe las credenciales mediante variables de Railway. Las credenciales no se guardan en GitHub ni en `.env.example`.
- Las fotos se almacenan en el Bucket; el navegador no recibe credenciales S3. El backend valida permisos y sirve la foto o emite una URL firmada de corta duración. En desarrollo se usa almacenamiento local detrás de la misma interfaz de la app.
- El backend expone HTTP y WebSocket para el frontend; Railway documenta soporte para WebSockets. Las variables de origen permitido y las cookies de sesión se configuran para los dominios reales de ambos servicios.
- Primero se despliega una versión mínima con una pantalla y una ruta de salud. Después se suben por etapas registro/perfil, contactos/chat y llamadas. Los cambios en `main` pueden activar el despliegue automático.
- La señalización WebRTC pasa por el backend, pero el audio va entre navegadores. Se prueba primero con STUN; si ciertas redes requieren TURN, se añade un servicio de retransmisión compatible fuera de Railway o en la VM de práctica.

Railway Buckets son almacenamiento S3 privado y cobran actualmente por GB almacenado; el tráfico que el backend reenvía desde el Bucket también puede contar como egreso del servicio. Revisaremos el uso y el saldo de Railway antes de habilitar el servicio para fotos.

## Decisiones visuales provisionales

El concepto anterior de marfil/serif fue rechazado. La versión actual usa una interfaz social oscura, densa y propia: grafito azulado `#11151D`–`#252E3A`, verde cítrico `#C7F36B` para selección y presencia, y coral `#FF7770` para finalizar llamadas. Mantiene una barra de navegación, una lista dedicada de conversaciones y el chat activo, con controles de voz acoplados sobre el compositor. **ENTRE** sigue siendo un nombre temporal. El prototipo en `mockups/chat-v2.html` es estático y no se conecta al backend ni a WebRTC.

## Orden de aprendizaje sugerido

1. Crear el repositorio GitHub de esta app con `main` y las carpetas `frontend/` y `backend/`, sin mezclar el laboratorio anterior.
2. Preparar Node/Rust y MySQL local; aprender Cargo y Axum con una ruta `/health` pequeña.
3. Conectar temprano el monorepo a un proyecto Railway independiente; el `Dockerfile` raíz construye frontend y backend en un despliegue.
4. Crear migraciones MySQL y registro, sesión, perfil, ID público y foto.
5. Añadir búsqueda por ID, solicitudes de contacto, conversaciones y mensajes.
6. Añadir presencia y señalización WebSocket; después WebRTC y pruebas de voz entre dispositivos.
7. Revisar créditos/uso de Railway, dominio y si la conexión entre redes requiere TURN.
