import { useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent, type FormEvent, type ReactNode } from 'react';
import { api, ApiError, type Account, type ChatMessage, type Contact, type ContactRequest, type ContactSearchResult } from './api';
import { useRealtime, type RealtimeEvent } from './useRealtime';
import './chat.css';

type AuthMode = 'register' | 'login';
type CallRole = 'caller' | 'callee';
type CallStatus = 'preparing' | 'ringing' | 'incoming' | 'connecting' | 'active';
type VoiceCall = {
  callId: string | null;
  peer: Account & { online: boolean };
  role: CallRole;
  status: CallStatus;
  startedAt: number | null;
  muted: boolean;
};
type IconName = 'chat' | 'people' | 'settings' | 'search' | 'plus' | 'clock' | 'pin' | 'phone' | 'user' | 'mic' | 'mic-off' | 'hangup' | 'send' | 'check' | 'close' | 'copy' | 'arrow' | 'logout';

const MAX_AVATAR_BYTES = 5 * 1024 * 1024;
const ALLOWED_AVATAR_TYPES = new Set(['image/jpeg', 'image/png', 'image/webp']);

function Icon({ name }: { name: IconName }) {
  const paths: Record<IconName, ReactNode> = {
    chat: <><path d="M20.2 11.3a7.8 7.8 0 0 1-8.2 7.3 8.8 8.8 0 0 1-3.2-.6L4 20l1.3-3.7a7 7 0 0 1-1.3-4.1A7.8 7.8 0 0 1 12.2 5a7.8 7.8 0 0 1 8 6.3Z"/><path d="M8.4 11.7h.1m3.4 0h.1m3.4 0h.1"/></>,
    people: <><path d="M16 20v-1.4a4.2 4.2 0 0 0-4.2-4.2H6.2A4.2 4.2 0 0 0 2 18.6V20"/><circle cx="9" cy="7" r="3.4"/><path d="M22 20v-1.4a4.2 4.2 0 0 0-3.2-4.1M16.4 3.7a3.4 3.4 0 0 1 0 6.6"/></>,
    settings: <><circle cx="12" cy="12" r="3"/><path d="m19.4 15 .1.1 1.2.9-1.2 2.1-1.4-.6a7.7 7.7 0 0 1-1.6.9l-.2 1.5h-2.5l-.3-1.5a7.7 7.7 0 0 1-1.7-.9l-1.3.6-1.3-2.1 1.2-.9a7.6 7.6 0 0 1 0-1.9l-1.2-.9 1.3-2.1 1.3.6c.5-.4 1.1-.7 1.7-.9l.3-1.5h2.5l.2 1.5c.6.2 1.2.5 1.6.9l1.4-.6 1.2 2.1-1.2.9a7.6 7.6 0 0 1-.1 1.8Z" transform="translate(-1 -1)"/></>,
    search: <><circle cx="10.8" cy="10.8" r="6.7"/><path d="m16 16 4.2 4.2"/></>,
    plus: <path d="M12 5v14M5 12h14"/>,
    clock: <><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></>,
    pin: <path d="m16 3 5 5-4 1-4 4-1 4-2-2-7 6 6-7-2-2 4-1 4-4 1-4Z"/>,
    phone: <path d="M21 16.5v3a2 2 0 0 1-2.2 2 19.8 19.8 0 0 1-8.6-3.1 19.4 19.4 0 0 1-6-6A19.8 19.8 0 0 1 1.1 3.7 2 2 0 0 1 3.1 1.5h3a2 2 0 0 1 2 1.7c.1 1 .4 2 .7 2.9a2 2 0 0 1-.5 2.1L7 9.5a16 16 0 0 0 6 6l1.3-1.3a2 2 0 0 1 2.1-.5c.9.3 1.9.6 2.9.7a2 2 0 0 1 1.7 2.1Z"/>,
    user: <><circle cx="12" cy="8" r="3.5"/><path d="M5 21v-1.5a7 7 0 0 1 14 0V21"/></>,
    mic: <><rect x="9" y="2" width="6" height="12" rx="3"/><path d="M5 11a7 7 0 0 0 14 0m-7 7v4m-4 0h8"/></>,
    'mic-off': <><path d="m2 2 20 20"/><path d="M9 9v3a3 3 0 0 0 5.1 2.1M15 9V5a3 3 0 0 0-5.8-1M5 11a7 7 0 0 0 12 4.9M19 11a7 7 0 0 1-.6 2.8M12 18v4m-4 0h8"/></>,
    hangup: <path d="M3.5 15.4a2 2 0 0 1-.3-3.1 13.2 13.2 0 0 1 17.6 0 2 2 0 0 1-.3 3.1l-2.3 1.4a2 2 0 0 1-2.7-.6l-.9-1.4a2 2 0 0 0-1.7-.9h-1.8a2 2 0 0 0-1.7.9l-.9 1.4a2 2 0 0 1-2.7.6l-2.3-1.4Z"/>,
    send: <><path d="m22 2-7 20-4-9-9-4 20-7Z"/><path d="M22 2 11 13"/></>,
    check: <path d="m5 12 4 4L19 6"/>,
    close: <path d="m6 6 12 12M18 6 6 18"/>,
    copy: <><rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3"/></>,
    arrow: <><path d="M4 12h15M12 5l7 7-7 7"/></>,
    logout: <><path d="M10 17l5-5-5-5M15 12H3"/><path d="M12 3h6a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-6"/></>,
  };
  return <svg className="icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">{paths[name]}</svg>;
}

function initials(name: string) {
  return name.trim().split(/\s+/).slice(0, 2).map((part) => part[0]?.toLocaleUpperCase('es-CR') ?? '').join('') || '?';
}

function Avatar({ account, className }: { account: Account; className: string }) {
  if (account.avatar_url) return <img className={className} src={account.avatar_url} alt="" loading="lazy" />;
  return <span className={`${className} avatar-fallback`} aria-hidden="true">{initials(account.display_name)}</span>;
}

function formatTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return new Intl.DateTimeFormat('es-CR', { hour: '2-digit', minute: '2-digit' }).format(date);
}

function formatDuration(startedAt: number | null, now: number) {
  if (!startedAt) return '00:00';
  const total = Math.max(0, Math.floor((now - startedAt) / 1000));
  return `${String(Math.floor(total / 60)).padStart(2, '0')}:${String(total % 60).padStart(2, '0')}`;
}

function isUserCard(value: unknown): value is Account & { online: boolean } {
  return typeof value === 'object' && value !== null
    && 'public_id' in value && typeof value.public_id === 'string'
    && 'display_name' in value && typeof value.display_name === 'string'
    && 'online' in value && typeof value.online === 'boolean'
    && 'avatar_url' in value;
}

function isChatMessage(value: unknown): value is ChatMessage {
  return typeof value === 'object' && value !== null
    && 'id' in value && typeof value.id === 'number'
    && 'sender_public_id' in value && typeof value.sender_public_id === 'string'
    && 'body' in value && typeof value.body === 'string'
    && 'sent_at' in value && typeof value.sent_at === 'string'
    && 'mine' in value && typeof value.mine === 'boolean';
}

function App() {
  const [account, setAccount] = useState<Account | null>(null);
  const [checkingSession, setCheckingSession] = useState(true);
  const [authMode, setAuthMode] = useState<AuthMode>('register');
  const [authName, setAuthName] = useState('');
  const [authEmail, setAuthEmail] = useState('');
  const [authPassword, setAuthPassword] = useState('');
  const [authAvatar, setAuthAvatar] = useState<File | null>(null);
  const [authAvatarUrl, setAuthAvatarUrl] = useState<string | null>(null);
  const [authBusy, setAuthBusy] = useState(false);
  const [authNotice, setAuthNotice] = useState('');
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [incomingRequests, setIncomingRequests] = useState<ContactRequest[]>([]);
  const [outgoingRequests, setOutgoingRequests] = useState<ContactRequest[]>([]);
  const [selectedPublicId, setSelectedPublicId] = useState<string | null>(null);
  const [messageThread, setMessageThread] = useState<{ conversationId: number | null; items: ChatMessage[] }>({ conversationId: null, items: [] });
  const [messageText, setMessageText] = useState('');
  const [messageLoading, setMessageLoading] = useState(false);
  const [messageSending, setMessageSending] = useState(false);
  const [sidebarQuery, setSidebarQuery] = useState('');
  const [peopleView, setPeopleView] = useState(false);
  const [mobileSidebarOpen, setMobileSidebarOpen] = useState(false);
  const [contactModalOpen, setContactModalOpen] = useState(false);
  const [contactId, setContactId] = useState('');
  const [searchingContact, setSearchingContact] = useState(false);
  const [searchResult, setSearchResult] = useState<ContactSearchResult | null>(null);
  const [searchNotice, setSearchNotice] = useState('');
  const [profileModalOpen, setProfileModalOpen] = useState(false);
  const [profileName, setProfileName] = useState('');
  const [profileAvatar, setProfileAvatar] = useState<File | null>(null);
  const [profileAvatarUrl, setProfileAvatarUrl] = useState<string | null>(null);
  const [profileBusy, setProfileBusy] = useState(false);
  const [profileNotice, setProfileNotice] = useState('');
  const [toast, setToast] = useState('');
  const [voiceCall, setVoiceCall] = useState<VoiceCall | null>(null);
  const [callNow, setCallNow] = useState(Date.now());

  const selectedContact = contacts.find((contact) => contact.public_id === selectedPublicId) ?? null;
  const messages = selectedContact?.conversation_id === messageThread.conversationId ? messageThread.items : [];
  const callRef = useRef(voiceCall);
  const accountRef = useRef(account);
  const contactsRef = useRef(contacts);
  const selectedRef = useRef(selectedPublicId);
  const signalRef = useRef<(event: Record<string, unknown>) => boolean>(() => false);
  const eventHandlerRef = useRef<(event: RealtimeEvent) => void>(() => undefined);
  const peerConnectionRef = useRef<RTCPeerConnection | null>(null);
  const localStreamRef = useRef<MediaStream | null>(null);
  const pendingCandidatesRef = useRef<RTCIceCandidateInit[]>([]);
  const remoteAudioRef = useRef<HTMLAudioElement | null>(null);
  const messagesEndRef = useRef<HTMLDivElement | null>(null);
  const toastTimerRef = useRef<number | null>(null);

  callRef.current = voiceCall;
  accountRef.current = account;
  contactsRef.current = contacts;
  selectedRef.current = selectedPublicId;

  const showToast = useCallback((message: string) => {
    setToast(message);
    if (toastTimerRef.current !== null) window.clearTimeout(toastTimerRef.current);
    toastTimerRef.current = window.setTimeout(() => setToast(''), 3600);
  }, []);

  const refreshContacts = useCallback(async () => {
    if (!accountRef.current) return;
    const result = await api.contacts();
    setContacts(result.contacts);
    setIncomingRequests(result.incoming_requests);
    setOutgoingRequests(result.outgoing_requests);
    setSelectedPublicId((current) => result.contacts.some((contact) => contact.public_id === current)
      ? current
      : result.contacts[0]?.public_id ?? null);
  }, []);

  const handleRealtimeEvent = useCallback((event: RealtimeEvent) => {
    if (event.type === 'contact:request' || event.type === 'contact:accepted' || event.type === 'contact:rejected') {
      void refreshContacts().catch(() => undefined);
      if (event.type === 'contact:request') showToast('Te llegó una solicitud de contacto.');
      if (event.type === 'contact:accepted') showToast('Ya pueden conversar y llamarse.');
      return;
    }
    if (event.type === 'presence' && typeof event.public_id === 'string' && typeof event.online === 'boolean') {
      setContacts((current) => current.map((contact) => contact.public_id === event.public_id
        ? { ...contact, online: event.online as boolean }
        : contact));
      return;
    }
    if (event.type === 'message:new' && typeof event.conversation_id === 'number' && isChatMessage(event.message)) {
      const message = event.message;
      setContacts((current) => current.map((contact) => contact.conversation_id === event.conversation_id
        ? { ...contact, last_message: message.body, last_message_at: message.sent_at }
        : contact));
      const active = contactsRef.current.find((contact) => contact.public_id === selectedRef.current);
      if (active?.conversation_id === event.conversation_id) {
        setMessageThread((current) => {
          const existing = current.conversationId === event.conversation_id ? current.items : [];
          return current.conversationId === event.conversation_id && existing.some((item) => item.id === message.id)
            ? current
            : { conversationId: event.conversation_id as number, items: [...existing, message] };
        });
        if (!message.mine) showToast(`${message.sender_name}: ${message.body.slice(0, 48)}`);
      }
      return;
    }
    if (event.type === 'call:incoming' && typeof event.call_id === 'string' && isUserCard(event.from)) {
      setVoiceCall({ callId: event.call_id, peer: event.from, role: 'callee', status: 'incoming', startedAt: null, muted: false });
      return;
    }
    if (event.type === 'call:outgoing' && typeof event.call_id === 'string' && isUserCard(event.to)) {
      setVoiceCall({ callId: event.call_id, peer: event.to, role: 'caller', status: 'ringing', startedAt: null, muted: false });
      return;
    }
    if (event.type === 'call:accepted' && typeof event.call_id === 'string') {
      const current = callRef.current;
      if (!current || current.callId !== event.call_id) return;
      setVoiceCall({ ...current, status: 'connecting' });
      if (current.role === 'caller') void createOffer(event.call_id);
      return;
    }
    if ((event.type === 'call:offer' || event.type === 'call:answer') && typeof event.call_id === 'string' && event.payload) {
      void applyDescription(event.type, event.call_id, event.payload);
      return;
    }
    if (event.type === 'call:ice' && typeof event.call_id === 'string' && event.payload) {
      void addIceCandidate(event.call_id, event.payload);
      return;
    }
    if (event.type === 'call:declined') {
      finishCall(false);
      showToast('La otra persona rechazó la llamada.');
      return;
    }
    if (event.type === 'call:missed') {
      finishCall(false);
      showToast('La llamada quedó sin respuesta.');
      return;
    }
    if (event.type === 'call:ended') {
      finishCall(false);
      showToast('La llamada terminó.');
      return;
    }
    if (event.type === 'call:error' && typeof event.message === 'string') {
      finishCall(false);
      showToast(event.message);
      return;
    }
    if (event.type === 'error' && typeof event.message === 'string') showToast(event.message);
  // The callback uses refs for current call state so the WebSocket handler stays stable.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refreshContacts, showToast]);

  eventHandlerRef.current = handleRealtimeEvent;
  const { send: sendRealtime, status: realtimeStatus } = useRealtime(Boolean(account), (event) => eventHandlerRef.current(event));
  signalRef.current = sendRealtime;

  useEffect(() => {
    let cancelled = false;
    api.me().then((user) => { if (!cancelled) setAccount(user); })
      .catch((error: unknown) => {
        if (!cancelled && !(error instanceof ApiError && error.status === 401)) {
          showToast(error instanceof Error ? error.message : 'No se pudo revisar la sesión.');
        }
      })
      .finally(() => { if (!cancelled) setCheckingSession(false); });
    return () => { cancelled = true; };
  }, [showToast]);

  useEffect(() => {
    if (!account) return;
    void refreshContacts().catch((error: unknown) => showToast(error instanceof Error ? error.message : 'No se pudieron cargar los contactos.'));
  }, [account, refreshContacts, showToast]);

  useEffect(() => {
    if (!selectedContact) {
      setMessageThread({ conversationId: null, items: [] });
      return;
    }
    const conversationId = selectedContact.conversation_id;
    let cancelled = false;
    setMessageLoading(true);
    api.messages(conversationId)
      .then((result) => {
        if (cancelled) return;
        setMessageThread((current) => {
          const existing = current.conversationId === conversationId ? current.items : [];
          const merged = new Map(result.map((message) => [message.id, message]));
          existing.forEach((message) => merged.set(message.id, message));
          return { conversationId, items: [...merged.values()].sort((a, b) => a.id - b.id) };
        });
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setMessageThread({ conversationId, items: [] });
          showToast(error instanceof Error ? error.message : 'No se pudieron cargar los mensajes.');
        }
      })
      .finally(() => { if (!cancelled) setMessageLoading(false); });
    return () => { cancelled = true; };
  }, [selectedContact?.conversation_id, showToast]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' });
  }, [messages]);

  useEffect(() => {
    if (voiceCall?.status !== 'active') return;
    const timer = window.setInterval(() => setCallNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [voiceCall?.status]);

  useEffect(() => {
    if (!authAvatar) { setAuthAvatarUrl(null); return; }
    const url = URL.createObjectURL(authAvatar);
    setAuthAvatarUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [authAvatar]);

  useEffect(() => {
    if (!profileAvatar) { setProfileAvatarUrl(null); return; }
    const url = URL.createObjectURL(profileAvatar);
    setProfileAvatarUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [profileAvatar]);

  useEffect(() => () => {
    if (toastTimerRef.current !== null) window.clearTimeout(toastTimerRef.current);
    peerConnectionRef.current?.close();
    localStreamRef.current?.getTracks().forEach((track) => track.stop());
  }, []);

  async function preparePeerConnection() {
    if (peerConnectionRef.current) return peerConnectionRef.current;
    if (!navigator.mediaDevices?.getUserMedia) throw new Error('Este navegador no permite usar el micrófono. Abrí el enlace en HTTPS.');
    const [{ iceServers }, stream] = await Promise.all([
      api.iceConfig(),
      navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true }, video: false }),
    ]);
    const peer = new RTCPeerConnection({ iceServers });
    stream.getTracks().forEach((track) => peer.addTrack(track, stream));
    peer.onicecandidate = (event) => {
      const callId = callRef.current?.callId;
      if (event.candidate && callId) signalRef.current({ type: 'call:ice', call_id: callId, candidate: event.candidate.toJSON() });
    };
    peer.ontrack = (event) => {
      const audio = remoteAudioRef.current;
      if (!audio) return;
      audio.srcObject = event.streams[0] ?? new MediaStream([event.track]);
      void audio.play().catch(() => showToast('Tocá el control de audio del navegador para escuchar la llamada.'));
    };
    peer.onconnectionstatechange = () => {
      if (peer.connectionState === 'connected') {
        setVoiceCall((current) => current ? { ...current, status: 'active', startedAt: current.startedAt ?? Date.now() } : current);
        setCallNow(Date.now());
      } else if (peer.connectionState === 'failed') {
        showToast('No se pudo establecer el audio. Revisá la conexión o la configuración TURN.');
      }
    };
    peerConnectionRef.current = peer;
    localStreamRef.current = stream;
    return peer;
  }

  async function createOffer(callId: string) {
    try {
      const peer = await preparePeerConnection();
      const offer = await peer.createOffer();
      await peer.setLocalDescription(offer);
      signalRef.current({ type: 'call:offer', call_id: callId, sdp: peer.localDescription });
    } catch {
      showToast('No se pudo preparar la llamada. Revisá el permiso del micrófono e intentá otra vez.');
      finishCall(true);
    }
  }

  async function applyDescription(kind: 'call:offer' | 'call:answer', callId: string, value: unknown) {
    if (callRef.current?.callId !== callId || typeof value !== 'object' || value === null || !('type' in value) || !('sdp' in value)) return;
    try {
      const peer = await preparePeerConnection();
      await peer.setRemoteDescription(value as RTCSessionDescriptionInit);
      const pending = pendingCandidatesRef.current.splice(0);
      for (const candidate of pending) await peer.addIceCandidate(candidate);
      if (kind === 'call:offer') {
        const answer = await peer.createAnswer();
        await peer.setLocalDescription(answer);
        signalRef.current({ type: 'call:answer', call_id: callId, sdp: peer.localDescription });
      }
    } catch {
      showToast('Falló la negociación del audio. Finalizá e intentá llamar de nuevo.');
      finishCall(true);
    }
  }

  async function addIceCandidate(callId: string, value: unknown) {
    if (callRef.current?.callId !== callId || typeof value !== 'object' || value === null || !('candidate' in value)) return;
    const candidate = value as RTCIceCandidateInit;
    const peer = peerConnectionRef.current;
    if (!peer?.remoteDescription) { pendingCandidatesRef.current.push(candidate); return; }
    try { await peer.addIceCandidate(candidate); } catch { /* ICE may arrive after a peer has disconnected. */ }
  }

  function finishCall(notifyPeer: boolean) {
    const current = callRef.current;
    if (notifyPeer && current?.callId) signalRef.current({ type: 'call:end', call_id: current.callId });
    peerConnectionRef.current?.close();
    peerConnectionRef.current = null;
    localStreamRef.current?.getTracks().forEach((track) => track.stop());
    localStreamRef.current = null;
    pendingCandidatesRef.current = [];
    if (remoteAudioRef.current) remoteAudioRef.current.srcObject = null;
    setVoiceCall(null);
  }

  async function placeCall(contact: Contact) {
    if (realtimeStatus !== 'online') { showToast('Conectando el servicio en vivo; probá llamar en un momento.'); return; }
    if (callRef.current) { showToast('Ya hay una llamada en curso.'); return; }
    setVoiceCall({ callId: null, peer: contact, role: 'caller', status: 'preparing', startedAt: null, muted: false });
    try {
      await preparePeerConnection();
      if (!signalRef.current({ type: 'call:invite', target_public_id: contact.public_id })) {
        throw new Error('Se perdió la conexión en vivo.');
      }
    } catch (error) {
      finishCall(false);
      showToast(error instanceof Error ? error.message : 'No se pudo acceder al micrófono.');
    }
  }

  async function acceptIncomingCall() {
    const current = callRef.current;
    if (!current || current.status !== 'incoming' || !current.callId) return;
    setVoiceCall({ ...current, status: 'preparing' });
    try {
      await preparePeerConnection();
      if (!signalRef.current({ type: 'call:accept', call_id: current.callId })) throw new Error('Se perdió la conexión en vivo.');
      setVoiceCall((value) => value ? { ...value, status: 'connecting' } : value);
    } catch (error) {
      signalRef.current({ type: 'call:decline', call_id: current.callId });
      finishCall(false);
      showToast(error instanceof Error ? error.message : 'No se pudo acceder al micrófono.');
    }
  }

  function declineIncomingCall() {
    const current = callRef.current;
    if (current?.callId) signalRef.current({ type: 'call:decline', call_id: current.callId });
    finishCall(false);
  }

  function toggleMute() {
    const current = callRef.current;
    if (!current) return;
    const muted = !current.muted;
    localStreamRef.current?.getAudioTracks().forEach((track) => { track.enabled = !muted; });
    setVoiceCall({ ...current, muted });
  }

  async function submitAuth(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setAuthBusy(true);
    setAuthNotice('');
    try {
      let user: Account;
      if (authMode === 'register') {
        const result = await api.register(authName, authEmail, authPassword);
        user = result.user;
        if (authAvatar) {
          try { user = await api.uploadAvatar(authAvatar); }
          catch { showToast('La cuenta quedó creada; no pudimos guardar la foto. Podés subirla desde tu perfil.'); }
        }
        showToast(`Cuenta lista. Tu ID es ${user.public_id}; lo copiás desde tu perfil.`);
      } else {
        user = (await api.login(authEmail, authPassword)).user;
        showToast(`Qué bueno verte, ${user.display_name}.`);
      }
      setAccount(user);
      setAuthPassword('');
      setAuthAvatar(null);
    } catch (error) {
      setAuthNotice(error instanceof Error ? error.message : 'No se pudo completar el acceso.');
    } finally {
      setAuthBusy(false);
    }
  }

  function selectAuthAvatar(event: ChangeEvent<HTMLInputElement>) {
    const file = event.currentTarget.files?.[0] ?? null;
    if (!file) { setAuthAvatar(null); return; }
    if (!ALLOWED_AVATAR_TYPES.has(file.type)) { setAuthNotice('Usá una foto JPG, PNG o WebP.'); event.currentTarget.value = ''; return; }
    if (file.size > MAX_AVATAR_BYTES) { setAuthNotice('La foto puede pesar hasta 5 MB.'); event.currentTarget.value = ''; return; }
    setAuthNotice('');
    setAuthAvatar(file);
  }

  function openProfile() {
    if (!account) return;
    setProfileName(account.display_name);
    setProfileAvatar(null);
    setProfileNotice('');
    setProfileModalOpen(true);
  }

  async function saveProfile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!account) return;
    setProfileBusy(true);
    setProfileNotice('');
    try {
      let updated = await api.updateProfile(profileName);
      if (profileAvatar) updated = await api.uploadAvatar(profileAvatar);
      setAccount(updated);
      setProfileModalOpen(false);
      showToast('Perfil actualizado.');
    } catch (error) {
      setProfileNotice(error instanceof Error ? error.message : 'No se pudo guardar el perfil.');
    } finally {
      setProfileBusy(false);
    }
  }

  async function removeProfilePhoto() {
    try {
      await api.deleteAvatar();
      setAccount((current) => current ? { ...current, avatar_url: null } : current);
      setProfileAvatar(null);
      setProfileNotice('Foto eliminada.');
    } catch (error) {
      setProfileNotice(error instanceof Error ? error.message : 'No se pudo quitar la foto.');
    }
  }

  async function copyAccountId() {
    if (!account) return;
    try { await navigator.clipboard.writeText(account.public_id); showToast('ID copiado.'); }
    catch { showToast(`Tu ID es ${account.public_id}. Copialo desde acá.`); }
  }

  async function submitContactSearch(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSearchingContact(true);
    setSearchNotice('');
    setSearchResult(null);
    try { setSearchResult(await api.searchUser(contactId)); }
    catch (error) { setSearchNotice(error instanceof Error ? error.message : 'No se pudo buscar ese ID.'); }
    finally { setSearchingContact(false); }
  }

  async function sendContactRequest() {
    if (!searchResult) return;
    try {
      await api.requestContact(searchResult.user.public_id);
      setContactModalOpen(false);
      setContactId('');
      setSearchResult(null);
      await refreshContacts();
      showToast('Solicitud enviada. La otra persona debe aceptarla.');
    } catch (error) { setSearchNotice(error instanceof Error ? error.message : 'No se pudo enviar la solicitud.'); }
  }

  async function respondToRequest(request: ContactRequest, accept: boolean) {
    try {
      if (accept) await api.acceptContact(request.id);
      else await api.rejectContact(request.id);
      await refreshContacts();
      if (accept) setSelectedPublicId(request.user.public_id);
      showToast(accept ? `Ya podés conversar con ${request.user.display_name}.` : 'Solicitud rechazada.');
    } catch (error) { showToast(error instanceof Error ? error.message : 'No se pudo procesar la solicitud.'); }
  }

  async function submitMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedContact || !messageText.trim() || messageSending) return;
    const body = messageText.trim();
    setMessageSending(true);
    try {
      const message = await api.sendMessage(selectedContact.conversation_id, body);
      setMessageThread((current) => {
        const existing = current.conversationId === selectedContact.conversation_id ? current.items : [];
        return current.conversationId === selectedContact.conversation_id && existing.some((item) => item.id === message.id)
          ? current
          : { conversationId: selectedContact.conversation_id, items: [...existing, message] };
      });
      setContacts((current) => current.map((contact) => contact.public_id === selectedContact.public_id
        ? { ...contact, last_message: message.body, last_message_at: message.sent_at }
        : contact));
      setMessageText('');
    } catch (error) { showToast(error instanceof Error ? error.message : 'No se pudo enviar el mensaje.'); }
    finally { setMessageSending(false); }
  }

  async function logout() {
    try { await api.logout(); }
    catch { /* Clear the local view even if the server already expired the session. */ }
    finishCall(true);
    setAccount(null);
    setContacts([]);
    setMessageThread({ conversationId: null, items: [] });
    setSelectedPublicId(null);
    setProfileModalOpen(false);
    setAuthMode('login');
  }

  const filteredContacts = useMemo(() => {
    const query = sidebarQuery.trim().toLocaleLowerCase('es-CR');
    return [...contacts]
      .filter((contact) => !query || `${contact.display_name} ${contact.public_id}`.toLocaleLowerCase('es-CR').includes(query))
      .sort((a, b) => Number(b.online) - Number(a.online) || (b.last_message_at ?? '').localeCompare(a.last_message_at ?? ''));
  }, [contacts, sidebarQuery]);

  if (checkingSession) {
    return <main className="app-loading"><span className="loading-mark"><i /><i /></span><span>Abriendo tu espacio…</span></main>;
  }

  if (!account) {
    return (
      <main className="page-shell">
        <header className="topbar">
          <a className="brand" href="/" aria-label="ENTRE, inicio"><BrandMark /><span>ENTRE</span><span className="brand-divider" /><span className="brand-caption">MENSAJES PRIVADOS · VOZ</span></a>
          <div className="topbar-meta"><span className="service-indicator"><i /> {realtimeStatus === 'online' ? 'EN LÍNEA' : 'TU ESPACIO'}</span></div>
        </header>
        <section className="main-grid" aria-labelledby="page-title">
          <div className="intro-column">
            <div className="eyebrow"><span className="eyebrow-line" /> CONVERSACIONES ENTRE PERSONAS</div>
            <h1 id="page-title">Hablá con<br />quien <em>elegís.</em></h1>
            <p className="intro-copy">Encontrá a alguien por su ID, conectá con su permiso y pasen del mensaje a la voz.</p>
            <div className="feature-list" aria-label="Funciones">
              <div className="feature-row"><span className="feature-number">01</span><span className="feature-name">Un ID para encontrarse</span><span className="feature-detail">privado · compartible</span></div>
              <div className="feature-row"><span className="feature-number">02</span><span className="feature-name">Mensajes entre contactos aceptados</span><span className="feature-detail">con permiso</span></div>
              <div className="feature-row"><span className="feature-number">03</span><span className="feature-name">Llamadas de voz 1 a 1</span><span className="feature-detail">en tiempo real</span></div>
            </div>
            <ConnectionDiagram />
            <p className="intro-footnote">Tu correo se queda privado. Tu ID lo compartís vos.</p>
          </div>

          <section className="account-card" aria-label={authMode === 'register' ? 'Crear cuenta' : 'Ingresar'}>
            <div className="card-topline"><span className="card-kicker">TU ESPACIO</span><span className="card-step">{authMode === 'register' ? '01 / 02' : '02 / 02'}</span></div>
            <h2>{authMode === 'register' ? 'Empezá por vos.' : 'Qué bueno verte.'}</h2>
            <p className="card-intro">{authMode === 'register' ? 'Creá tu cuenta; después te damos un ID para compartir.' : 'Ingresá para volver a tus conversaciones.'}</p>
            <div className="mode-switch" role="tablist" aria-label="Acceso">
              <button className={authMode === 'register' ? 'mode-tab active' : 'mode-tab'} type="button" role="tab" aria-selected={authMode === 'register'} onClick={() => { setAuthMode('register'); setAuthNotice(''); }}>Crear cuenta</button>
              <button className={authMode === 'login' ? 'mode-tab active' : 'mode-tab'} type="button" role="tab" aria-selected={authMode === 'login'} onClick={() => { setAuthMode('login'); setAuthNotice(''); }}>Ingresar</button>
            </div>
            <form className="account-form" onSubmit={submitAuth} aria-busy={authBusy}>
              {authMode === 'register' && <>
                <div className="avatar-field">
                  <label className="avatar-picker" htmlFor="auth-avatar" aria-label="Elegir foto de perfil">
                    {authAvatarUrl ? <img src={authAvatarUrl} alt="Vista previa de la foto" /> : <span className="avatar-placeholder">+</span>}
                    <span className="avatar-camera" aria-hidden="true"><Icon name="user" /></span>
                    <input id="auth-avatar" type="file" accept="image/jpeg,image/png,image/webp" onChange={selectAuthAvatar} />
                  </label>
                  <div className="avatar-instructions"><span className="field-label">FOTO DE PERFIL <span>OPCIONAL</span></span><label htmlFor="auth-avatar" className="avatar-action">{authAvatar ? 'Cambiar foto' : 'Elegir una foto'}</label><span className="avatar-hint">JPG, PNG o WebP · hasta 5 MB</span></div>
                </div>
                <label className="field-group"><span className="field-label">NOMBRE VISIBLE</span><input value={authName} onChange={(event) => setAuthName(event.target.value)} type="text" autoComplete="name" placeholder="¿Cómo te van a ver?" minLength={2} maxLength={40} required /></label>
              </>}
              <label className="field-group"><span className="field-label">CORREO ELECTRÓNICO</span><input value={authEmail} onChange={(event) => setAuthEmail(event.target.value)} type="email" autoComplete="email" placeholder="vos@correo.com" maxLength={254} required /></label>
              <label className="field-group"><span className="field-label">CONTRASEÑA</span><input value={authPassword} onChange={(event) => setAuthPassword(event.target.value)} type="password" autoComplete={authMode === 'register' ? 'new-password' : 'current-password'} placeholder={authMode === 'register' ? 'Mínimo 12 caracteres' : 'Tu contraseña'} minLength={authMode === 'register' ? 12 : 1} maxLength={128} required /></label>
              <button className="submit-button" type="submit" disabled={authBusy}>{authBusy ? 'Un momento…' : authMode === 'register' ? 'Crear mi cuenta' : 'Ingresar'}<Icon name="arrow" /></button>
              <p className={`form-notice ${authNotice ? 'has-error' : ''}`} aria-live="polite">{authNotice || (authMode === 'register' ? 'La contraseña se guarda como hash; nunca en texto claro.' : 'Si todavía no tenés cuenta, creala en la pestaña anterior.')}</p>
            </form>
            <div className="card-bottom"><span className="lock-icon" aria-hidden="true"><Icon name="settings" /></span><span>Solo tus contactos aceptados pueden escribirte o llamarte.</span></div>
          </section>
        </section>
        <footer className="page-footer"><span>ENTRE <span className="footer-muted">/ NOMBRE PROVISIONAL</span></span><span>CHAT PRIVADO · VOZ 1 A 1</span><a href="/health" target="_blank" rel="noreferrer">ESTADO DEL SERVIDOR</a></footer>
        {toast && <div className="toast auth-toast" role="status" aria-live="polite">{toast}</div>}
      </main>
    );
  }

  return (
    <div className={`app-shell ${mobileSidebarOpen ? 'mobile-sidebar-open' : ''}`}>
      <nav className="rail" aria-label="Navegación principal">
        <div className="brand-mark" aria-label="ENTRE"><svg className="icon" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M5 6h7M5 12h10M5 18h6"/><path d="m14 5 5 7-5 7"/></svg></div>
        <button className={`rail-button ${!peopleView ? 'active' : ''}`} type="button" aria-label="Mensajes" title="Mensajes" onClick={() => { setPeopleView(false); setMobileSidebarOpen(true); }}><Icon name="chat" /></button>
        <button className={`rail-button ${peopleView ? 'active' : ''}`} type="button" aria-label="Personas" title="Personas" onClick={() => { setPeopleView(true); setContactModalOpen(true); }}><Icon name="people" /></button>
        <div className="rail-rule" />
        <div className="rail-spacer" />
        <button className="rail-button" type="button" aria-label="Perfil y ajustes" title="Perfil y ajustes" onClick={openProfile}><Icon name="settings" /></button>
      </nav>

      <aside className="sidebar" aria-label="Conversaciones y contactos">
        <div className="sidebar-head">
          <div className="brand-row"><div className="wordmark"><span className="wordmark-mark" aria-hidden="true"><i /><i /></span>ENTRE</div><button className="icon-button new-message" type="button" aria-label="Agregar persona" title="Agregar persona" onClick={() => { setContactModalOpen(true); setSearchResult(null); setContactId(''); setSearchNotice(''); }}><Icon name="plus" /></button></div>
          <div className="section-title-row"><h1 className="section-title">{peopleView ? 'Personas' : 'Mensajes directos'}</h1><span className="list-count">{contacts.length.toString().padStart(2, '0')}</span></div>
          <label className="search-wrap"><Icon name="search" /><input className="search-input" value={sidebarQuery} onChange={(event) => setSidebarQuery(event.target.value)} type="search" placeholder="Buscar persona" autoComplete="off" aria-label="Buscar entre contactos" /></label>
        </div>

        <div className="sidebar-scroll">
          {incomingRequests.length > 0 && <section className="list-section request-section" aria-label="Solicitudes recibidas">
            <div className="list-section-head"><span><Icon name="people" />Solicitudes</span><span className="request-count">{incomingRequests.length}</span></div>
            {incomingRequests.map((request) => <div className="request-row" key={request.id}>
              <Avatar account={request.user} className="request-avatar" />
              <div className="request-copy"><strong>{request.user.display_name}</strong><span>{request.user.public_id}</span><div className="request-actions"><button type="button" className="request-accept" onClick={() => void respondToRequest(request, true)} aria-label={`Aceptar a ${request.user.display_name}`}><Icon name="check" />Aceptar</button><button type="button" className="request-reject" onClick={() => void respondToRequest(request, false)} aria-label={`Rechazar a ${request.user.display_name}`}><Icon name="close" /></button></div></div>
            </div>)}
          </section>}

          <section className="list-section" aria-label="Contactos">
            <div className="list-section-head"><span><Icon name={peopleView ? 'people' : 'clock'} />{peopleView ? 'Contactos aceptados' : 'Recientes'}</span><button className="icon-button small-add" type="button" aria-label="Agregar contacto" title="Agregar contacto" onClick={() => setContactModalOpen(true)}><Icon name="plus" /></button></div>
            {filteredContacts.map((contact) => <button className={`contact-row ${selectedPublicId === contact.public_id ? 'selected' : ''}`} type="button" key={contact.public_id} onClick={() => { setSelectedPublicId(contact.public_id); setMobileSidebarOpen(false); setPeopleView(false); }} aria-current={selectedPublicId === contact.public_id ? 'true' : undefined}>
              <span className="avatar-wrap"><Avatar account={contact} className="contact-avatar" /><i className={`avatar-status ${contact.online ? 'online' : ''}`} /></span>
              <span className="contact-meta"><span className="contact-topline"><span className="contact-name">{contact.display_name}</span><span className="contact-time">{contact.last_message_at ? formatTime(contact.last_message_at) : ''}</span></span><span className="contact-preview">{contact.last_message || (contact.online ? 'Disponible ahora' : contact.public_id)}</span></span>
            </button>)}
            {filteredContacts.length === 0 && <div className="empty-search">{contacts.length ? 'No hay coincidencias.' : 'Todavía no agregaste personas.'}</div>}
            {outgoingRequests.length > 0 && <section className="outgoing-section" aria-label="Solicitudes enviadas"><div className="list-section-head"><span>Enviadas</span></div>{outgoingRequests.map((request) => <div className="outgoing-row" key={request.id}><span>{request.user.display_name}</span><span>Esperando respuesta</span></div>)}</section>}
          </section>
        </div>

        <div className="side-foot">
          <button className="self-avatar-button" type="button" onClick={openProfile} aria-label="Abrir perfil"><Avatar account={account} className="rail-avatar" /></button>
          <button className="self-copy" type="button" onClick={openProfile}><span className="self-name">{account.display_name}</span><span className="self-status">ID · {account.public_id}</span></button>
          <div className="foot-tools"><button className="icon-button" type="button" aria-label="Copiar mi ID" title="Copiar mi ID" onClick={() => void copyAccountId()}><Icon name="copy" /></button><button className="icon-button" type="button" aria-label="Cerrar sesión" title="Cerrar sesión" onClick={() => void logout()}><Icon name="logout" /></button></div>
        </div>
      </aside>
      <button className="drawer-backdrop" type="button" aria-label="Cerrar lista de contactos" onClick={() => setMobileSidebarOpen(false)} />

      <main className="main-pane">
        {selectedContact ? <>
          <header className="chat-header">
            <div className="chat-person"><Avatar account={selectedContact} className="contact-avatar" /><div className="chat-name-block"><h2 className="chat-title">{selectedContact.display_name}</h2><div className="chat-presence"><span className={`presence-dot ${selectedContact.online ? 'online' : ''}`} /><span>{selectedContact.online ? 'En línea' : 'Desconectado'}</span></div></div></div>
            <div className="header-tools"><button className="call-start" type="button" disabled={!selectedContact.online || realtimeStatus !== 'online' || Boolean(voiceCall)} onClick={() => void placeCall(selectedContact)}><Icon name="phone" /><span>Llamar</span></button><span className="header-divider" aria-hidden="true" /><span className={`realtime-label ${realtimeStatus}`}>{realtimeStatus === 'online' ? 'CONECTADO' : 'RECONECTANDO'}</span><button className="icon-button profile-view" type="button" aria-label="Ver perfil" title="Ver contacto" onClick={() => showToast(`ID de ${selectedContact.display_name}: ${selectedContact.public_id}`)}><Icon name="user" /></button></div>
          </header>
          <section className="messages" aria-label={`Conversación con ${selectedContact.display_name}`}>
            <div className="thread-inner">
              <div className="date-separator"><span>MENSAJES PRIVADOS</span></div>
              {messageLoading ? <div className="thread-state">Cargando conversación…</div> : messages.length === 0 ? <div className="thread-empty"><span className="thread-empty-mark"><Icon name="chat" /></span><h3>Acá empieza la conversación.</h3><p>Solo vos y {selectedContact.display_name} pueden ver estos mensajes.</p></div> : messages.map((message) => <article className={`message-group ${message.mine ? 'mine' : ''}`} key={message.id}>
                <Avatar account={message.mine ? account : selectedContact} className="message-avatar" />
                <div className="message-content"><div className="message-heading"><span className={`message-author ${message.mine ? 'self' : ''}`}>{message.mine ? 'Vos' : message.sender_name}</span><time className="message-time" dateTime={message.sent_at}>{formatTime(message.sent_at)}</time></div><p className="message-text">{message.body}</p></div>
              </article>)}
              <div ref={messagesEndRef} />
            </div>
          </section>
          {voiceCall && voiceCall.status !== 'incoming' && <section className={`call-dock ${voiceCall.status === 'active' ? 'call-live' : ''}`} aria-label="Llamada de voz">
            <Avatar account={voiceCall.peer} className="call-avatar" />
            <div className="call-summary"><div className="call-caption"><span className="call-state" /><span>{voiceCall.status === 'ringing' ? `Llamando a ${voiceCall.peer.display_name}` : `Llamada con ${voiceCall.peer.display_name}`}</span></div><div className="call-subline"><span>{voiceCall.status === 'active' ? 'Conectada' : voiceCall.status === 'ringing' ? 'Esperando respuesta' : voiceCall.status === 'preparing' ? 'Preparando micrófono' : 'Conectando audio'}</span><span aria-hidden="true">·</span><time className="call-timer">{formatDuration(voiceCall.startedAt, callNow)}</time><span className="voice-bars" aria-hidden="true"><i /><i /><i /><i /><i /></span></div></div>
            <div className="call-controls" role="group" aria-label="Controles de llamada"><button className={`call-control ${voiceCall.muted ? 'toggled' : ''}`} type="button" onClick={toggleMute} aria-label={voiceCall.muted ? 'Activar micrófono' : 'Silenciar micrófono'}><Icon name={voiceCall.muted ? 'mic-off' : 'mic'} /><span>{voiceCall.muted ? 'Activar' : 'Silenciar'}</span></button><button className="call-control end" type="button" onClick={() => finishCall(true)} aria-label="Finalizar llamada"><Icon name="hangup" /><span>Finalizar</span></button></div>
          </section>}
          <form className="composer-wrap" onSubmit={submitMessage}>
            <div className="composer"><textarea value={messageText} onChange={(event) => setMessageText(event.target.value.slice(0, 4000))} onKeyDown={(event) => { if (event.key === 'Enter' && !event.shiftKey) { event.preventDefault(); event.currentTarget.form?.requestSubmit(); } }} rows={1} placeholder={`Escribí un mensaje a ${selectedContact.display_name}`} aria-label="Escribí un mensaje" maxLength={4000} /><button className="send-button" type="submit" aria-label="Enviar mensaje" title="Enviar mensaje" disabled={messageSending || !messageText.trim()}><Icon name="send" /></button></div>
            <div className="composer-hint">Enter para enviar <span aria-hidden="true">·</span> Shift + Enter para nueva línea</div>
          </form>
        </> : <section className="empty-workspace"><div className="empty-workspace-art"><span /><span /><span /></div><p className="empty-kicker">TU ESPACIO, A TU MANERA</p><h2>Una conversación<br />a la vez.</h2><p>Agregá a alguien con su ID. Cuando acepte, van a poder escribir y llamarse.</p><button className="call-start" type="button" onClick={() => setContactModalOpen(true)}><Icon name="plus" /><span>Agregar persona</span></button><div className="own-id-line"><span>Tu ID</span><code>{account.public_id}</code><button className="icon-button" type="button" onClick={() => void copyAccountId()} aria-label="Copiar mi ID"><Icon name="copy" /></button></div></section>}
        <audio ref={remoteAudioRef} autoPlay playsInline className="remote-audio" aria-label="Audio de la llamada" />
      </main>

      {contactModalOpen && <div className="modal-scrim" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setContactModalOpen(false); }}>
        <section className="app-modal contact-modal" role="dialog" aria-modal="true" aria-labelledby="contact-modal-title"><div className="modal-heading"><div><span className="modal-kicker">NUEVO CONTACTO</span><h2 id="contact-modal-title">¿A quién querés encontrar?</h2></div><button className="icon-button" type="button" onClick={() => setContactModalOpen(false)} aria-label="Cerrar"><Icon name="close" /></button></div>
          <p className="modal-copy">Pedile su ID público. El correo no se usa para buscar personas.</p>
          <form className="contact-search-form" onSubmit={submitContactSearch}><label className="field-group"><span className="field-label">ID DE 8 CARACTERES</span><input value={contactId} onChange={(event) => { setContactId(event.target.value.toUpperCase().replace(/[^A-HJ-NP-Z2-9]/g, '').slice(0, 8)); setSearchResult(null); setSearchNotice(''); }} placeholder="7K4P2D9Q" autoComplete="off" maxLength={8} required /></label><button className="modal-primary" type="submit" disabled={searchingContact || contactId.length !== 8}>{searchingContact ? 'Buscando…' : 'Buscar'}</button></form>
          {searchNotice && <p className="modal-error" role="alert">{searchNotice}</p>}
          {searchResult && <div className="search-result"><Avatar account={searchResult.user} className="search-result-avatar" /><div className="search-result-copy"><strong>{searchResult.user.display_name}</strong><span>ID · {searchResult.user.public_id}</span></div>{searchResult.relationship === 'none' ? <button className="modal-primary" type="button" onClick={() => void sendContactRequest()}>Enviar solicitud</button> : <span className="relationship-label">{searchResult.relationship === 'accepted' ? 'Ya son contactos' : searchResult.relationship === 'incoming' ? 'Te envió una solicitud' : 'Solicitud enviada'}</span>}</div>}
          <div className="modal-footnote">La otra persona debe aceptar antes de que puedan conversar o llamarse.</div>
        </section>
      </div>}

      {profileModalOpen && <div className="modal-scrim" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setProfileModalOpen(false); }}>
        <section className="app-modal profile-modal" role="dialog" aria-modal="true" aria-labelledby="profile-modal-title"><div className="modal-heading"><div><span className="modal-kicker">TU PERFIL</span><h2 id="profile-modal-title">Tu identidad en ENTRE.</h2></div><button className="icon-button" type="button" onClick={() => setProfileModalOpen(false)} aria-label="Cerrar"><Icon name="close" /></button></div>
          <form onSubmit={saveProfile} className="profile-form">
            <div className="profile-photo-row"><Avatar account={{ ...account, avatar_url: profileAvatarUrl ?? account.avatar_url }} className="profile-avatar" /><div><span className="field-label">FOTO DE PERFIL</span><label className="avatar-action profile-file-action" htmlFor="profile-avatar-file">{profileAvatar ? 'Cambiar selección' : 'Elegir una foto'}</label><input className="visually-hidden" id="profile-avatar-file" type="file" accept="image/jpeg,image/png,image/webp" onChange={(event) => { const file = event.currentTarget.files?.[0] ?? null; if (file && !ALLOWED_AVATAR_TYPES.has(file.type)) { setProfileNotice('Usá una foto JPG, PNG o WebP.'); event.currentTarget.value = ''; return; } if (file && file.size > MAX_AVATAR_BYTES) { setProfileNotice('La foto puede pesar hasta 5 MB.'); event.currentTarget.value = ''; return; } setProfileNotice(''); setProfileAvatar(file); }} /><span className="avatar-hint">Se guarda en tamaño reducido.</span>{account.avatar_url && <button className="text-button" type="button" onClick={() => void removeProfilePhoto()}>Quitar foto</button>}</div></div>
            <label className="field-group"><span className="field-label">NOMBRE VISIBLE</span><input value={profileName} onChange={(event) => setProfileName(event.target.value)} minLength={2} maxLength={40} required /></label>
            <div className="profile-id-card"><div><span className="field-label">TU ID PÚBLICO</span><code>{account.public_id}</code></div><button className="copy-id-button" type="button" onClick={() => void copyAccountId()}><Icon name="copy" />Copiar ID</button></div>
            {profileNotice && <p className="modal-error" role="alert">{profileNotice}</p>}
            <div className="profile-actions"><button className="modal-secondary" type="button" onClick={() => setProfileModalOpen(false)}>Cancelar</button><button className="modal-primary" type="submit" disabled={profileBusy}>{profileBusy ? 'Guardando…' : 'Guardar cambios'}</button></div>
          </form>
          <button className="logout-link" type="button" onClick={() => void logout()}><Icon name="logout" />Cerrar sesión</button>
        </section>
      </div>}

      {voiceCall?.status === 'incoming' && <div className="modal-scrim call-scrim"><section className="app-modal incoming-call-card" role="dialog" aria-modal="true" aria-labelledby="incoming-title"><Avatar account={voiceCall.peer} className="incoming-avatar" /><span className="incoming-pulse" /><span className="modal-kicker">LLAMADA ENTRANTE</span><h2 id="incoming-title">{voiceCall.peer.display_name}</h2><p>Te está llamando por voz.</p><div className="incoming-actions"><button className="call-decline" type="button" onClick={declineIncomingCall}><Icon name="hangup" /><span>Rechazar</span></button><button className="call-accept" type="button" onClick={() => void acceptIncomingCall()}><Icon name="phone" /><span>Aceptar</span></button></div></section></div>}

      <div className="toast" role="status" aria-live="polite" data-visible={Boolean(toast)}>{toast}</div>
    </div>
  );
}

function BrandMark() {
  return <span className="brand-mark" aria-hidden="true"><svg viewBox="0 0 42 42" fill="none"><path d="M9 10.5h24v5H14v5h15v5H14v6H9V10.5Z" fill="currentColor"/><path d="M27 26.5h6v5h-6z" fill="#C7F36B"/></svg></span>;
}

function ConnectionDiagram() {
  return <div className="connection-diagram" aria-hidden="true"><svg className="connection-lines" viewBox="0 0 440 280" fill="none"><path d="M78 173 173 92l94 94 95-69"/><path d="M78 173h189M173 92l189 25"/></svg><span className="connection-node node-left"><span className="node-avatar avatar-c">C</span></span><span className="connection-node node-top"><span className="node-avatar avatar-d">D</span></span><span className="connection-node node-center"><BrandMark /></span><span className="connection-node node-right"><span className="node-avatar avatar-s">S</span></span><span className="diagram-label label-left">CAMILA · EN LÍNEA</span><span className="diagram-label label-right">ID · 7K4P2D9Q</span><span className="diagram-caption">Una solicitud aceptada.<br />Una conversación que empieza.</span></div>;
}

export default App;
