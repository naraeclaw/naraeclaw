import { useState, useEffect, useCallback } from 'react';
import { useAuth } from '../hooks/useAuth';

const API_BASE = ''; // Vite proxy handles /api/*

interface WikiEntry {
  key: string;
  content: string;
  category?: string;
  created_at?: string;
}

export default function Wiki() {
  const { token } = useAuth();
  const [entries, setEntries] = useState<WikiEntry[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [content, setContent] = useState('');
  const [search, setSearch] = useState('');
  const [editing, setEditing] = useState(false);
  const [newTitle, setNewTitle] = useState('');
  const [msg, setMsg] = useState('');

  const headers = useCallback((): Record<string, string> => {
    const h: Record<string, string> = { 'Content-Type': 'application/json' };
    if (token) h['Authorization'] = `Bearer ${token}`;
    return h;
  }, [token]);

  const load = useCallback(async () => {
    try {
      const url = search.trim()
        ? `${API_BASE}/api/memory?category=wiki&query=${encodeURIComponent(search)}`
        : `${API_BASE}/api/memory?category=wiki`;
      const resp = await fetch(url, { headers: headers() });
      if (!resp.ok) return;
      const data = await resp.json();
      setEntries(data.entries || []);
    } catch {}
  }, [search, headers]);

  useEffect(() => { load(); }, [load]);

  const openEntry = (entry: WikiEntry) => {
    setSelected(entry.key);
    setContent(entry.content);
    setEditing(false);
    setMsg('');
  };

  const save = async () => {
    if (!selected) return;
    try {
      const resp = await fetch(`${API_BASE}/api/memory`, {
        method: 'POST',
        headers: headers(),
        body: JSON.stringify({ key: selected, content, category: 'wiki' }),
      });
      if (resp.ok) {
        setMsg('저장됨');
        setEditing(false);
        await load();
      } else {
        setMsg('저장 실패');
      }
    } catch (e: any) {
      setMsg(`오류: ${e}`);
    }
  };

  const deleteEntry = async (key: string) => {
    try {
      await fetch(`${API_BASE}/api/memory/${encodeURIComponent(key)}`, {
        method: 'DELETE',
        headers: headers(),
      });
      if (selected === key) { setSelected(null); setContent(''); }
      await load();
    } catch {}
  };

  const createPage = async () => {
    if (!newTitle.trim()) return;
    const key = `wiki/${newTitle.toLowerCase().replace(/\s+/g, '-')}`;
    const md = `# ${newTitle}\n\n`;
    try {
      const resp = await fetch(`${API_BASE}/api/memory`, {
        method: 'POST',
        headers: headers(),
        body: JSON.stringify({ key, content: md, category: 'wiki' }),
      });
      if (resp.ok) {
        setNewTitle('');
        await load();
        setSelected(key);
        setContent(md);
        setEditing(true);
      }
    } catch {}
  };

  const title = (entry: WikiEntry) => {
    const firstLine = entry.content.split('\n').find(l => l.startsWith('# '));
    return firstLine?.replace(/^#\s+/, '') || entry.key.replace('wiki/', '');
  };

  return (
    <div style={{ display: 'flex', height: '100%', minHeight: 'calc(100vh - 60px)' }}>
      {/* Sidebar */}
      <div style={{ width: 260, borderRight: '1px solid #2a2a3e', padding: 16, overflowY: 'auto' }}>
        <input placeholder="검색..." value={search} onChange={e => setSearch(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && load()} style={{ ...inputStyle, width: '100%', marginBottom: 8 }} />
        <div style={{ display: 'flex', gap: 6, marginBottom: 12 }}>
          <input placeholder="새 페이지 제목" value={newTitle} onChange={e => setNewTitle(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && createPage()} style={{ ...inputStyle, flex: 1 }} />
          <button onClick={createPage} disabled={!newTitle.trim()} style={btnSmall}>+</button>
        </div>
        {entries.map(e => (
          <div key={e.key} onClick={() => openEntry(e)} style={{
            padding: '8px 12px', borderRadius: 6, cursor: 'pointer', marginBottom: 4,
            background: selected === e.key ? '#2a2a3e' : 'transparent',
          }}>
            <div style={{ fontSize: 14, fontWeight: selected === e.key ? 600 : 400 }}>{title(e)}</div>
            <div style={{ fontSize: 11, color: '#666', marginTop: 2 }}>{e.content.slice(0, 50).replace(/^#.*\n/, '')}</div>
          </div>
        ))}
        {entries.length === 0 && <p style={{ color: '#666', fontSize: 13 }}>페이지가 없습니다</p>}
      </div>

      {/* Content */}
      <div style={{ flex: 1, padding: 24 }}>
        {selected ? (
          <>
            <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 16 }}>
              <h2 style={{ fontSize: 18, margin: 0 }}>{title(entries.find(e => e.key === selected) || { key: selected, content: '' })}</h2>
              <div style={{ display: 'flex', gap: 8 }}>
                {editing ? (
                  <button onClick={save} style={btnPrimary}>저장</button>
                ) : (
                  <button onClick={() => setEditing(true)} style={btnSmall}>편집</button>
                )}
                <button onClick={() => deleteEntry(selected)} style={{ ...btnSmall, color: '#e94560' }}>삭제</button>
              </div>
            </div>
            {editing ? (
              <textarea value={content} onChange={e => setContent(e.target.value)} style={{
                width: '100%', height: 'calc(100vh - 200px)', background: '#0f0f1a', border: '1px solid #2a2a3e',
                borderRadius: 8, color: '#e0e0e0', padding: 16, fontSize: 14, fontFamily: 'monospace',
                resize: 'none', boxSizing: 'border-box',
              }} />
            ) : (
              <div style={{ background: '#1a1a2e', padding: 20, borderRadius: 8, whiteSpace: 'pre-wrap', fontSize: 14, lineHeight: 1.6 }}>
                {content}
              </div>
            )}
          </>
        ) : (
          <div style={{ textAlign: 'center', color: '#666', marginTop: 80 }}>
            <p style={{ fontSize: 32, marginBottom: 8 }}>📝</p>
            <p>페이지를 선택하거나 새로 만드세요</p>
          </div>
        )}
        {msg && <p style={{ marginTop: 12, fontSize: 13, color: '#4a9eff' }}>{msg}</p>}
      </div>
    </div>
  );
}

const inputStyle: React.CSSProperties = { padding: '6px 10px', background: '#0f0f1a', border: '1px solid #2a2a3e', borderRadius: 6, color: '#e0e0e0', fontSize: 13, boxSizing: 'border-box' as const };
const btnPrimary: React.CSSProperties = { background: '#4a9eff', color: '#fff', border: 'none', borderRadius: 6, padding: '6px 14px', cursor: 'pointer', fontSize: 13 };
const btnSmall: React.CSSProperties = { background: '#2a2a3e', color: '#ccc', border: '1px solid #3a3a4e', borderRadius: 6, padding: '6px 14px', fontSize: 13, cursor: 'pointer' };
