import { useState, useEffect } from 'react';
import { isTauri } from '../lib/tauri';

interface RemoteServer {
  id: string;
  name: string;
  url: string;
  token: string | null;
  connected: boolean;
}

export default function RemoteServers() {
  const [servers, setServers] = useState<RemoteServer[]>([]);
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');
  const [token, setToken] = useState('');
  const [msg, setMsg] = useState('');

  useEffect(() => { load(); }, []);

  const load = async () => {
    if (!isTauri()) return;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const list = await invoke<RemoteServer[]>('list_servers');
      setServers(list);
    } catch {}
  };

  const add = async () => {
    if (!isTauri()) { setMsg('Tauri 환경에서만 가능합니다'); return; }
    setMsg('연결 확인 중...');
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('add_server', { name, url, token: token || null });
      setAdding(false);
      setName(''); setUrl(''); setToken('');
      await load();
      setMsg('서버 추가됨');
    } catch (e: any) {
      setMsg(`${e}`);
    }
  };

  const remove = async (id: string) => {
    if (!isTauri()) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('remove_server', { serverId: id });
    await load();
  };

  const switchTo = async (s: RemoteServer) => {
    if (!isTauri()) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('switch_server', { url: s.url, token: s.token });
    setMsg(`${s.name}으로 전환됨`);
  };

  const switchLocal = async () => {
    if (!isTauri()) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('switch_server', { url: 'http://127.0.0.1:42617', token: null });
    setMsg('로컬로 전환됨');
  };

  return (
    <div style={{ padding: 24, maxWidth: 600 }}>
      <h2 style={{ fontSize: 20, marginBottom: 4 }}>원격 서버</h2>
      <p style={{ color: '#888', fontSize: 14, marginBottom: 20 }}>다른 서버의 NaraeClaw에 연결하여 원격 관리</p>

      <div style={{ ...cardStyle, border: '1px solid #4a9eff' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <div><strong>🏠 로컬</strong> <span style={{ fontSize: 12, color: '#888' }}>127.0.0.1:42617</span></div>
          <button onClick={switchLocal} style={btnSmall}>전환</button>
        </div>
      </div>

      {servers.map(s => (
        <div key={s.id} style={cardStyle}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <div>
              <strong>{s.name}</strong>
              <span style={{ marginLeft: 8, fontSize: 12, color: s.connected ? '#4a9eff' : '#e94560' }}>
                {s.connected ? '● 연결됨' : '● 연결 실패'}
              </span>
              <div style={{ fontSize: 12, color: '#666', marginTop: 2 }}>{s.url}</div>
            </div>
            <div style={{ display: 'flex', gap: 6 }}>
              <button onClick={() => switchTo(s)} style={btnSmall}>전환</button>
              <button onClick={() => remove(s.id)} style={{ ...btnSmall, color: '#e94560' }}>삭제</button>
            </div>
          </div>
        </div>
      ))}

      {servers.length === 0 && (
        <div style={{ ...cardStyle, textAlign: 'center', color: '#666' }}>
          <p>등록된 원격 서버가 없습니다</p>
        </div>
      )}

      {!adding ? (
        <button onClick={() => setAdding(true)} style={{ ...btnPrimary, marginTop: 12 }}>+ 서버 추가</button>
      ) : (
        <div style={{ ...cardStyle, marginTop: 12 }}>
          <input placeholder="이름 (예: 내 서버)" value={name} onChange={e => setName(e.target.value)} style={inputStyle} />
          <input placeholder="URL (예: http://192.168.1.10:42617)" value={url} onChange={e => setUrl(e.target.value)} style={{ ...inputStyle, marginTop: 8 }} />
          <input placeholder="토큰 (선택)" type="password" value={token} onChange={e => setToken(e.target.value)} style={{ ...inputStyle, marginTop: 8 }} />
          <div style={{ marginTop: 8, display: 'flex', gap: 8 }}>
            <button onClick={add} disabled={!name || !url} style={{ ...btnPrimary, opacity: name && url ? 1 : 0.5 }}>추가</button>
            <button onClick={() => setAdding(false)} style={btnSmall}>취소</button>
          </div>
        </div>
      )}
      {msg && <p style={{ marginTop: 12, fontSize: 14, color: '#4a9eff' }}>{msg}</p>}
    </div>
  );
}

const cardStyle: React.CSSProperties = { background: '#1a1a2e', padding: 16, borderRadius: 8, marginBottom: 8 };
const inputStyle: React.CSSProperties = { width: '100%', padding: '8px 12px', background: '#0f0f1a', border: '1px solid #2a2a3e', borderRadius: 6, color: '#e0e0e0', fontSize: 14, boxSizing: 'border-box' as const };
const btnPrimary: React.CSSProperties = { background: '#4a9eff', color: '#fff', border: 'none', borderRadius: 6, padding: '8px 16px', cursor: 'pointer' };
const btnSmall: React.CSSProperties = { background: '#2a2a3e', color: '#ccc', border: '1px solid #3a3a4e', borderRadius: 6, padding: '6px 14px', fontSize: 13, cursor: 'pointer' };
