# Arquitectura de ENTRE

> Nombre provisional. Implementación inicial para conversaciones privadas y llamadas de voz 1:1.

## Flujo del producto

Cada persona registra correo, contraseña y nombre visible; puede añadir una imagen. La API asigna un ID público aleatorio de ocho caracteres. Una persona busca a la otra por ID y envía una solicitud. Solo al aceptarla se crea la conversación y se habilitan mensajes y llamadas. Para llamar, ambas personas deben tener sesión y estar conectadas.

## Frontend

`frontend/` contiene React + TypeScript + Vite. `App.tsx` coordina registro, perfiles, contactos, solicitudes, mensajes y controles de voz. `api.ts` maneja peticiones HTTP con cookie de sesión; `useRealtime.ts` mantiene el WebSocket; `chat.css` conserva el diseño visual personalizado basado en `mockups/chat-v2.html`.

## Backend

`backend/src/main.rs` conecta MySQL, aplica migraciones y arranca Axum. `lib.rs` configura rutas, límites de cuerpo, origen y cabeceras; `auth.rs` gestiona cuentas, sesiones y fotos; `social.rs` aplica las reglas de contactos y mensajes; `realtime.rs` administra presencia, invitaciones y señalización WebRTC.

Axum sirve tanto la API como el build de React. El mismo origen permite mantener cookies y WebSocket en un solo dominio.

## Datos de MySQL

- `users`: correo privado, nombre visible, ID público, hash Argon2 y avatar JPEG procesado.
- `sessions`: hash del token de sesión y vencimiento. El token sin hash solo viaja en cookie `HttpOnly`, `SameSite=Lax` y `Secure` en HTTPS.
- `contact_requests`: dirección de la solicitud y estado `pending`, `accepted` o `rejected`.
- `conversations`: una conversación por pareja, con identificadores ordenados.
- `messages`: texto, remitente, conversación y hora; cada consulta verifica que la persona sea miembro.

El avatar de entrada admite JPEG, PNG o WebP hasta 5 MB, limita dimensiones, elimina metadatos al recodificarlo y guarda el JPEG de hasta 512 px en la fila de usuario. El archivo original no se conserva.

## Chat y presencia

Los mensajes se guardan en MySQL y se envían en vivo a ambos usuarios por WebSocket. La presencia y los estados activos se mantienen en memoria del proceso; no se deben desplegar varias réplicas sin reemplazar el Hub por pub/sub compartido. El historial que entrega la pantalla inicial es el bloque más reciente de hasta 100 mensajes.

## Llamadas

La API autentica el WebSocket y comprueba que solo contactos aceptados puedan llamarse. El servidor envía la invitación y reenvía oferta, respuesta y candidatos ICE; el navegador solicita permiso de micrófono y establece audio WebRTC. No hay video ni grabación y el estado de una llamada activa no persiste al reiniciar el proceso.

STUN puede bastar cuando ambos navegadores tienen rutas compatibles. Redes con NAT restrictivo requieren TURN. La app puede solicitar credenciales temporales a Cloudflare Realtime TURN usando `CF_TURN_KEY_ID` y `CF_TURN_API_TOKEN`, que permanecen en Railway y nunca se envían al navegador como secretos duraderos.

## Seguridad y límites del MVP

- Contraseñas con Argon2; tokens de sesión aleatorios con hash guardado en MySQL.
- Validación del `Origin` para cambios de estado y WebSocket, límite de frecuencia para registro, login, búsqueda, contactos y avatares.
- Las fotos se sirven con autenticación y solo a la propia persona o a un contacto aceptado.
- Sin grupos, video, restablecimiento de contraseña, verificación por correo ni adjuntos.
- Un solo servicio web de Railway y una réplica; MySQL conserva el estado persistente.