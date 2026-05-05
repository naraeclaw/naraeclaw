import { useState, useEffect, useRef, useCallback } from 'react';
import { AlertCircle } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { WsMessage } from '@/types/api';
import { WebSocketClient, getOrCreateSessionId } from '@/lib/ws';
import { generateUUID } from '@/lib/uuid';
import { useDraft } from '@/hooks/useDraft';
import { t } from '@/lib/i18n';
import { getSessionMessages } from '@/lib/api';
import ToolCallCard from '@/components/ToolCallCard';
import type { ToolCallInfo } from '@/components/ToolCallCard';
import {
  loadChatHistory,
  mapServerMessagesToPersisted,
  persistedToUiMessages,
  saveChatHistory,
  uiMessagesToPersisted,
} from '@/lib/chatHistoryStorage';

interface ChatMessage {
  id: string;
  role: 'user' | 'agent';
  content: string;
  thinking?: string;
  markdown?: boolean;
  toolCall?: ToolCallInfo;
  timestamp: Date;
}

const DRAFT_KEY = 'agent-chat';

// SVG icon paths for the composer
const ICON_ATTACH = '<path d="M21.4 11.05l-9.2 9.2a5.5 5.5 0 01-7.78-7.78l9.2-9.2a3.7 3.7 0 015.22 5.22l-9.2 9.2a1.85 1.85 0 01-2.61-2.61L14 7.7"/>';
const ICON_SEND = '<path d="M5 12l14-7-5 14-3-7-6 0z" fill="currentColor" stroke="none"/>';

function SvgIcon({ path, size = 14, sw = 1.7 }: { path: string; size?: number; sw?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor"
      strokeWidth={sw} strokeLinecap="round" strokeLinejoin="round"
      dangerouslySetInnerHTML={{ __html: path }} />
  );
}

export default function AgentChat() {
  const sessionIdRef = useRef(getOrCreateSessionId());
  const { draft, saveDraft, clearDraft } = useDraft(DRAFT_KEY);
  const [messages, setMessages] = useState<ChatMessage[]>(() => {
    const persisted = loadChatHistory(sessionIdRef.current);
    return persisted.length > 0 ? persistedToUiMessages(persisted) : [];
  });
  const [historyReady, setHistoryReady] = useState(false);
  const [input, setInput] = useState(draft);
  const [typing, setTyping] = useState(false);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);

  const wsRef = useRef<WebSocketClient | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const pendingContentRef = useRef('');
  const pendingThinkingRef = useRef('');
  const capturedThinkingRef = useRef('');
  const [streamingContent, setStreamingContent] = useState('');
  const [streamingThinking, setStreamingThinking] = useState('');

  useEffect(() => { saveDraft(input); }, [input, saveDraft]);

  useEffect(() => {
    const sid = sessionIdRef.current;
    let cancelled = false;
    (async () => {
      try {
        const res = await getSessionMessages(sid);
        if (cancelled) return;
        if (res.session_persistence && res.messages.length > 0) {
          setMessages(prev => prev.length > 0 ? prev : persistedToUiMessages(mapServerMessagesToPersisted(res.messages)));
        } else if (!res.session_persistence) {
          setMessages(prev => {
            if (prev.length > 0) return prev;
            const ls = loadChatHistory(sid);
            return ls.length ? persistedToUiMessages(ls) : prev;
          });
        }
      } catch {
        if (!cancelled) {
          setMessages(prev => {
            if (prev.length > 0) return prev;
            const ls = loadChatHistory(sid);
            return ls.length ? persistedToUiMessages(ls) : prev;
          });
        }
      } finally {
        if (!cancelled) setHistoryReady(true);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    if (!historyReady) return;
    saveChatHistory(sessionIdRef.current, uiMessagesToPersisted(messages));
  }, [messages, historyReady]);

  useEffect(() => {
    const ws = new WebSocketClient();

    ws.onOpen = () => { setConnected(true); setError(null); };
    ws.onClose = (ev: CloseEvent) => {
      setConnected(false);
      if (ev.code !== 1000 && ev.code !== 1001) setError(`연결이 종료되었습니다 (코드: ${ev.code})`);
    };
    ws.onError = () => { setError(t('agent.connection_error')); };

    ws.onMessage = (msg: WsMessage) => {
      switch (msg.type) {
        case 'session_start':
        case 'connected': break;

        case 'thinking':
          setTyping(true);
          pendingThinkingRef.current += msg.content ?? '';
          setStreamingThinking(pendingThinkingRef.current);
          break;

        case 'chunk':
          setTyping(true);
          pendingContentRef.current += msg.content ?? '';
          setStreamingContent(pendingContentRef.current);
          break;

        case 'chunk_reset':
          capturedThinkingRef.current = pendingThinkingRef.current;
          pendingContentRef.current = '';
          pendingThinkingRef.current = '';
          setStreamingContent('');
          setStreamingThinking('');
          break;

        case 'message':
        case 'done': {
          const content = msg.full_response ?? msg.content ?? pendingContentRef.current;
          const thinking = capturedThinkingRef.current || pendingThinkingRef.current || undefined;
          if (content) {
            setMessages(prev => [...prev, { id: generateUUID(), role: 'agent', content, thinking, markdown: true, timestamp: new Date() }]);
          }
          pendingContentRef.current = '';
          pendingThinkingRef.current = '';
          capturedThinkingRef.current = '';
          setStreamingContent('');
          setStreamingThinking('');
          setTyping(false);
          break;
        }

        case 'tool_call': {
          const toolName = msg.name ?? 'unknown';
          const toolArgs = msg.args;
          setMessages(prev => {
            const argsKey = JSON.stringify(toolArgs ?? {});
            const isDuplicate = prev.some(m => m.toolCall && m.toolCall.output === undefined && m.toolCall.name === toolName && JSON.stringify(m.toolCall.args ?? {}) === argsKey);
            if (isDuplicate) return prev;
            return [...prev, { id: generateUUID(), role: 'agent' as const, content: `${t('agent.tool_call_prefix')} ${toolName}(${argsKey})`, toolCall: { name: toolName, args: toolArgs }, timestamp: new Date() }];
          });
          break;
        }

        case 'tool_result': {
          setMessages(prev => {
            const idx = prev.findIndex(m => m.toolCall && m.toolCall.output === undefined);
            if (idx !== -1) {
              const updated = [...prev];
              const existing = prev[idx]!;
              updated[idx] = { ...existing, toolCall: { ...existing.toolCall!, output: msg.output ?? '' } };
              return updated;
            }
            return [...prev, { id: generateUUID(), role: 'agent' as const, content: `${t('agent.tool_result_prefix')} ${msg.output ?? ''}`, toolCall: { name: msg.name ?? 'unknown', output: msg.output ?? '' }, timestamp: new Date() }];
          });
          break;
        }

        case 'cron_result': {
          const cronOutput = msg.output ?? '';
          if (cronOutput) {
            setMessages(prev => [...prev, { id: generateUUID(), role: 'agent' as const, content: cronOutput, markdown: true, timestamp: new Date(msg.timestamp ?? Date.now()) }]);
          }
          break;
        }

        case 'error':
          setMessages(prev => [...prev, { id: generateUUID(), role: 'agent', content: `${t('agent.error_prefix')} ${msg.message ?? t('agent.unknown_error')}`, timestamp: new Date() }]);
          if (msg.code === 'AGENT_INIT_FAILED' || msg.code === 'AUTH_ERROR' || msg.code === 'PROVIDER_ERROR') {
            const errMsg = msg.message || '';
            setError(`설정 오류: ${errMsg}`);
            if (errMsg.includes('unable to load model')) {
              (async () => {
                try {
                  const { isTauri } = await import('../lib/tauri');
                  if (!isTauri()) return;
                  const { invoke } = await import('@tauri-apps/api/core');
                  const config = await invoke<Record<string, unknown>>('get_config');
                  const model = (config?.default_model as string) || '';
                  if (model) {
                    setError(`모델 손상 감지 — ${model} 자동 재설치 중...`);
                    await invoke('ollama_repair_model', { model });
                    setError(`${model} 재설치 완료. 다시 메시지를 보내보세요.`);
                  }
                } catch (e: unknown) {
                  setError(`모델 자동 재설치 실패: ${e}`);
                }
              })();
            }
          }
          setTyping(false);
          pendingContentRef.current = '';
          pendingThinkingRef.current = '';
          setStreamingContent('');
          setStreamingThinking('');
          break;
      }
    };

    ws.connect();
    wsRef.current = ws;
    return () => { ws.disconnect(); };
  }, []);

  useEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
  }, [messages, typing, streamingContent]);

  const handleSend = () => {
    const trimmed = input.trim();
    if (!trimmed || !wsRef.current?.connected) return;
    setMessages(prev => [...prev, { id: generateUUID(), role: 'user', content: trimmed, timestamp: new Date() }]);
    try {
      wsRef.current.sendMessage(trimmed);
      setTyping(true);
      pendingContentRef.current = '';
      pendingThinkingRef.current = '';
    } catch {
      setError(t('agent.send_error'));
    }
    setInput('');
    clearDraft();
    if (inputRef.current) { inputRef.current.style.height = 'auto'; inputRef.current.focus(); }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); handleSend(); }
    if (e.key === 'V' && (e.ctrlKey || e.metaKey) && e.shiftKey) {
      e.preventDefault();
      navigator.clipboard.readText().then(text => {
        if (!text.trim() || !wsRef.current || !connected) return;
        const msg = `클립보드 내용을 분석해줘:\n\n${text}`;
        wsRef.current!.sendMessage(msg);
        setMessages(prev => [...prev, { id: generateUUID(), role: 'user', content: msg, timestamp: new Date() }]);
      }).catch(() => { });
    }
  };

  const handleTextareaChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value);
    e.target.style.height = 'auto';
    e.target.style.height = `${Math.min(e.target.scrollHeight, 200)}px`;
  };

  const handleCopy = useCallback((msgId: string, content: string) => {
    const onSuccess = () => {
      setCopiedId(msgId);
      setTimeout(() => setCopiedId(prev => (prev === msgId ? null : prev)), 2000);
    };
    if (navigator.clipboard?.writeText) {
      navigator.clipboard.writeText(content).then(onSuccess).catch(() => {
        const ta = document.createElement('textarea');
        ta.value = content; ta.style.position = 'fixed'; ta.style.opacity = '0';
        document.body.appendChild(ta); ta.select();
        try { document.execCommand('copy'); onSuccess(); } finally { document.body.removeChild(ta); }
      });
    } else {
      const ta = document.createElement('textarea');
      ta.value = content; ta.style.position = 'fixed'; ta.style.opacity = '0';
      document.body.appendChild(ta); ta.select();
      try { document.execCommand('copy'); onSuccess(); } finally { document.body.removeChild(ta); }
    }
  }, []);

  const msgCount = messages.filter(m => !m.toolCall).length;

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', position: 'relative', minHeight: 0 }}>
      {/* Error bar */}
      {error && (
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 16px', background: 'rgba(195,64,67,0.10)', borderBottom: '1px solid rgba(195,64,67,0.25)', color: 'var(--color-status-error)', fontSize: 12.5 }}>
          <AlertCircle size={14} />
          <span style={{ flex: 1 }}>{error}</span>
          {error.includes('unable to load model') && (
            <button onClick={async () => {
              try {
                const { isTauri } = await import('../lib/tauri');
                if (!isTauri()) return;
                const { invoke } = await import('@tauri-apps/api/core');
                const config = await invoke<Record<string, unknown>>('get_config');
                const model = (config?.default_model as string) || '';
                if (model) { setError(`${model} 재설치 중...`); await invoke('ollama_repair_model', { model }); setError(null); }
              } catch (e: unknown) { setError(`재설치 실패: ${e}`); }
            }} style={{ padding: '3px 10px', borderRadius: 6, background: 'var(--pc-accent)', color: '#fff', border: 'none', cursor: 'pointer', fontSize: 11.5, fontWeight: 600 }}>
              재설치
            </button>
          )}
        </div>
      )}

      {/* Page header */}
      <div className="page-head" style={{ paddingBottom: 14 }}>
        <div style={{ flex: 1 }}>
          <div className="crumb">에이전트</div>
          <h1>대화</h1>
          <div className="sub">
            <span className="mono" style={{ color: 'var(--pc-text-secondary)' }}>{sessionIdRef.current.slice(0, 16)}</span>
            {' '}· {msgCount} 메시지
          </div>
        </div>
        <div style={{ display: 'flex', gap: 6 }}>
          <button
            onClick={() => handleCopy('all', messages.map(m => `[${m.role}] ${m.content}`).join('\n\n'))}
            title="대화 복사"
            style={{ padding: '6px 10px', borderRadius: 8, border: '1px solid var(--pc-border)', background: 'transparent', color: 'var(--pc-text-muted)', cursor: 'pointer', fontSize: 12 }}
          >
            {copiedId === 'all' ? '✓' : '복사'}
          </button>
          <button
            onClick={() => { setMessages([]); clearDraft(); setInput(''); }}
            style={{ padding: '6px 12px', borderRadius: 8, border: '1px solid var(--pc-border)', background: 'transparent', color: 'var(--pc-text-muted)', cursor: 'pointer', fontSize: 12 }}
          >
            새 세션
          </button>
        </div>
      </div>

      {/* Document-style message list */}
      <div
        ref={scrollRef}
        className="chat-doc"
        style={{ flex: 1, overflowY: 'auto' }}
        onDragOver={e => { e.preventDefault(); setDragOver(true); }}
        onDragLeave={() => setDragOver(false)}
        onDrop={async e => {
          e.preventDefault(); setDragOver(false);
          const files = Array.from(e.dataTransfer.files);
          for (const file of files) {
            const text = await file.text().catch(() => `[바이너리 파일: ${file.name}]`);
            const msg = `파일 '${file.name}'의 내용을 분석해줘:\n\n\`\`\`\n${text.slice(0, 10000)}\n\`\`\``;
            if (wsRef.current && connected) {
              wsRef.current.sendMessage(msg);
              setMessages(prev => [...prev, { id: generateUUID(), role: 'user', content: msg, timestamp: new Date() }]);
            }
          }
        }}
      >
        {dragOver && (
          <div style={{ textAlign: 'center', padding: '8px 0', fontSize: 12.5, color: 'var(--pc-accent)' }}>
            📎 파일을 놓으면 에이전트에게 전달됩니다
          </div>
        )}

        {messages.length === 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', gap: 12, color: 'var(--pc-text-muted)' }}>
            <div style={{ width: 56, height: 56, borderRadius: 16, background: 'var(--pc-bg-elevated)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 24 }}>나</div>
            <div style={{ textAlign: 'center' }}>
              <div style={{ fontSize: 15, fontWeight: 600, color: 'var(--pc-text-primary)', marginBottom: 4 }}>나래 에이전트</div>
              <div style={{ fontSize: 13 }}>{t('agent.start_conversation')}</div>
            </div>
          </div>
        )}

        {messages.map((msg, i) => (
          <div key={msg.id} className="msg-row fade-in">
            <div className="msg-author">
              <div className={`av ${msg.role}`}>
                {msg.role === 'user' ? '나' : 'N'}
              </div>
              {i < messages.length - 1 && <div className="line" />}
            </div>
            <div className="msg-body">
              <div className="msg-meta">
                <b>{msg.role === 'user' ? '사용자' : '나래'}</b>
                {' '}
                <span style={{ fontSize: 11, color: 'var(--pc-text-faint)' }}>
                  {msg.timestamp.toLocaleTimeString('ko-KR', { hour: '2-digit', minute: '2-digit' })}
                </span>
                <button
                  onClick={() => handleCopy(msg.id, msg.content)}
                  style={{ marginLeft: 'auto', opacity: 0, padding: '1px 6px', borderRadius: 5, border: '1px solid var(--pc-border)', background: 'transparent', color: 'var(--pc-text-muted)', cursor: 'pointer', fontSize: 11 }}
                  onMouseEnter={e => { (e.currentTarget as HTMLElement).style.opacity = '1'; }}
                  onMouseLeave={e => { (e.currentTarget as HTMLElement).style.opacity = '0'; }}
                >
                  {copiedId === msg.id ? '✓' : '복사'}
                </button>
              </div>
              <div className="msg-content">
                {msg.thinking && (
                  <details style={{ marginBottom: 8 }}>
                    <summary style={{ fontSize: 12, cursor: 'pointer', color: 'var(--pc-text-muted)' }}>생각 과정</summary>
                    <pre style={{ fontSize: 11.5, marginTop: 6, padding: '8px 12px', borderRadius: 8, background: 'var(--pc-bg-input)', color: 'var(--pc-text-secondary)', whiteSpace: 'pre-wrap', overflowX: 'auto' }}>
                      {msg.thinking}
                    </pre>
                  </details>
                )}
                {msg.toolCall ? (
                  <ToolCallCard toolCall={msg.toolCall} />
                ) : msg.markdown ? (
                  <div className="chat-markdown">
                    <ReactMarkdown remarkPlugins={[remarkGfm]}>{msg.content}</ReactMarkdown>
                  </div>
                ) : (
                  <p style={{ margin: 0, whiteSpace: 'pre-wrap' }}>{msg.content}</p>
                )}
              </div>
            </div>
          </div>
        ))}

        {/* Streaming / typing indicator */}
        {typing && (
          <div className="msg-row fade-in">
            <div className="msg-author">
              <div className="av agent">N</div>
            </div>
            <div className="msg-body">
              <div className="msg-meta"><b>나래</b></div>
              <div className="msg-content">
                {streamingThinking && (
                  <details style={{ marginBottom: 8 }} open={!streamingContent}>
                    <summary style={{ fontSize: 12, cursor: 'pointer', color: 'var(--pc-text-muted)' }}>생각 중{!streamingContent && '…'}</summary>
                    <pre style={{ fontSize: 11.5, marginTop: 6, padding: '8px 12px', borderRadius: 8, background: 'var(--pc-bg-input)', color: 'var(--pc-text-secondary)', whiteSpace: 'pre-wrap' }}>
                      {streamingThinking}
                    </pre>
                  </details>
                )}
                {streamingContent
                  ? <p style={{ margin: 0, whiteSpace: 'pre-wrap' }}>{streamingContent}</p>
                  : (
                    <div style={{ display: 'inline-flex', alignItems: 'center', gap: 6, marginTop: 4 }}>
                      <span className="typing"><span /><span /><span /></span>
                      <span className="tiny">생각하는 중</span>
                    </div>
                  )}
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Composer */}
      <div className="composer-wrap">
        <div className="composer">
          <textarea
            ref={inputRef}
            rows={1}
            placeholder={connected ? '나래에게 무엇이든 물어보세요…' : t('agent.connecting')}
            value={input}
            onChange={handleTextareaChange}
            onKeyDown={handleKeyDown}
            disabled={!connected}
            style={{ minHeight: 44, maxHeight: 200, paddingTop: 10, paddingBottom: 10 }}
          />
          <div className="composer-tools">
            <button title="파일 첨부" style={{ padding: '4px 8px', borderRadius: 7, border: '1px solid var(--pc-border)', background: 'transparent', color: 'var(--pc-text-muted)', cursor: 'pointer' }}>
              <SvgIcon path={ICON_ATTACH} size={14} />
            </button>
            <span className="pill" style={{ fontSize: 11 }}>claude-haiku-4-5</span>
            <span className="pill" style={{ fontSize: 11 }}>
              <span style={{ display: 'inline-block', width: 5, height: 5, borderRadius: '50%', background: connected ? 'var(--color-status-success)' : 'var(--color-status-error)', marginRight: 4, boxShadow: connected ? '0 0 5px var(--color-status-success)' : 'none', verticalAlign: 1 }} />
              {connected ? '연결됨' : '연결 중…'}
            </span>
            <div style={{ flex: 1 }} />
            <span className="kbd">⌘</span><span className="kbd">⏎</span>
            <button
              onClick={handleSend}
              disabled={!input.trim() || !connected}
              className="btn primary"
              style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '6px 14px' }}
            >
              <SvgIcon path={ICON_SEND} size={13} />
              보내기
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
