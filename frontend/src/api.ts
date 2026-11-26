export type Account = {
  public_id: string;
  display_name: string;
  avatar_url: string | null;
};

export type Contact = Account & {
  online: boolean;
  conversation_id: number;
  last_message: string | null;
  last_message_at: string | null;
};

export type ContactRequest = {
  id: number;
  user: Account & { online: boolean };
};

export type ContactSearchResult = {
  user: Account & { online: boolean };
  relationship: 'none' | 'incoming' | 'outgoing' | 'accepted';
};

export type ContactsResponse = {
  contacts: Contact[];
  incoming_requests: ContactRequest[];
  outgoing_requests: ContactRequest[];
};

export type ChatMessage = {
  id: number;
  sender_public_id: string;
  sender_name: string;
  body: string;
  sent_at: string;
  mine: boolean;
};

export class ApiError extends Error {
  constructor(message: string, readonly status: number) {
    super(message);
    this.name = 'ApiError';
  }
}

async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.body && !(init.body instanceof FormData) && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }
  const response = await fetch(`/api/v1${path}`, {
    ...init,
    headers,
    credentials: 'same-origin',
  });
  if (response.status === 204) return undefined as T;
  const payload: unknown = await response.json().catch(() => null);
  if (!response.ok) {
    const message = typeof payload === 'object' && payload !== null && 'message' in payload && typeof payload.message === 'string'
      ? payload.message
      : 'No se pudo completar la solicitud.';
    throw new ApiError(message, response.status);
  }
  return payload as T;
}

export const api = {
  me: () => request<Account>('/me'),
  register: (displayName: string, email: string, password: string) =>
    request<{ user: Account }>('/auth/register', { method: 'POST', body: JSON.stringify({ display_name: displayName, email, password }) }),
  login: (email: string, password: string) =>
    request<{ user: Account }>('/auth/login', { method: 'POST', body: JSON.stringify({ email, password }) }),
  logout: () => request<void>('/auth/logout', { method: 'POST' }),
  updateProfile: (displayName: string) =>
    request<Account>('/profile', { method: 'PATCH', body: JSON.stringify({ display_name: displayName }) }),
  uploadAvatar: (file: File) => {
    const form = new FormData();
    form.set('avatar', file);
    return request<Account>('/profile/avatar', { method: 'POST', body: form });
  },
  deleteAvatar: () => request<void>('/profile/avatar', { method: 'DELETE' }),
  contacts: () => request<ContactsResponse>('/contacts'),
  searchUser: (publicId: string) => request<ContactSearchResult>(`/users/search?public_id=${encodeURIComponent(publicId)}`),
  requestContact: (targetPublicId: string) =>
    request<void>('/contacts/requests', { method: 'POST', body: JSON.stringify({ target_public_id: targetPublicId }) }),
  acceptContact: (requestId: number) => request<void>(`/contacts/requests/${requestId}/accept`, { method: 'POST' }),
  rejectContact: (requestId: number) => request<void>(`/contacts/requests/${requestId}/reject`, { method: 'POST' }),
  messages: (conversationId: number) => request<ChatMessage[]>(`/conversations/${conversationId}/messages`),
  sendMessage: (conversationId: number, body: string) =>
    request<ChatMessage>(`/conversations/${conversationId}/messages`, { method: 'POST', body: JSON.stringify({ body }) }),
  iceConfig: () => request<{ iceServers: RTCIceServer[] }>('/calls/ice-config'),
};
