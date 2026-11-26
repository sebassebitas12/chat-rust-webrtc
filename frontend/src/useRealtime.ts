import { useCallback, useEffect, useRef, useState } from 'react';

export type RealtimeEvent = Record<string, unknown> & { type: string };

export function useRealtime(active: boolean, onEvent: (event: RealtimeEvent) => void) {
  const socketRef = useRef<WebSocket | null>(null);
  const eventHandlerRef = useRef(onEvent);
  const [status, setStatus] = useState<'offline' | 'connecting' | 'online'>('offline');

  useEffect(() => { eventHandlerRef.current = onEvent; }, [onEvent]);

  useEffect(() => {
    if (!active) {
      setStatus('offline');
      return;
    }

    let stopped = false;
    let retryTimer = 0;
    let retryCount = 0;
    let socket: WebSocket | null = null;

    const connect = () => {
      if (stopped) return;
      setStatus('connecting');
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      socket = new WebSocket(`${protocol}//${window.location.host}/api/v1/realtime`);
      socketRef.current = socket;
      socket.onopen = () => {
        retryCount = 0;
        setStatus('online');
      };
      socket.onmessage = (message) => {
        try {
          const event: unknown = JSON.parse(String(message.data));
          if (typeof event === 'object' && event !== null && 'type' in event && typeof event.type === 'string') {
            eventHandlerRef.current(event as RealtimeEvent);
          }
        } catch {
          // Ignore malformed frames; the server validates events before forwarding them.
        }
      };
      socket.onerror = () => socket?.close();
      socket.onclose = () => {
        if (socketRef.current === socket) socketRef.current = null;
        if (stopped) return;
        setStatus('connecting');
        const delay = Math.min(1000 * (2 ** retryCount), 20_000);
        retryCount += 1;
        retryTimer = window.setTimeout(connect, delay);
      };
    };

    connect();
    return () => {
      stopped = true;
      window.clearTimeout(retryTimer);
      socketRef.current = null;
      socket?.close();
      setStatus('offline');
    };
  }, [active]);

  const send = useCallback((event: Record<string, unknown>) => {
    if (socketRef.current?.readyState !== WebSocket.OPEN) return false;
    socketRef.current.send(JSON.stringify(event));
    return true;
  }, []);

  return { send, status };
}
