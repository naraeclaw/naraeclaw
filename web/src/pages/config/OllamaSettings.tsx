import { useState, useEffect } from 'react';
import { isTauri } from '../../lib/tauri';
import { invoke } from '@tauri-apps/api/core';

interface OllamaStatus {
  installed: boolean;
  running: boolean;
  models: string[];
}

export default function OllamaSettings() {
  const [status, setStatus] = useState<OllamaStatus | null>(null);
  const [pulling, setPulling] = useState(false);
  const [pullMsg, setPullMsg] = useState('');
  const [customModel, setCustomModel] = useState('');

  useEffect(() => { check(); }, []);

  const check = async () => {
    if (!isTauri()) return;
    try {
      const s = await invoke<OllamaStatus>('check_ollama');
      setStatus(s);
    } catch {}
  };

  const start = async () => {
    try { await invoke('ollama_start'); await check(); } catch {}
  };

  const pull = async (model: string) => {
    setPulling(true);
    setPullMsg(`${model} 다운로드 중...`);
    try {
      await invoke('ollama_pull', { model });
      setPullMsg(`${model} 완료!`);
      await check();
    } catch (e: any) {
      setPullMsg(`실패: ${e}`);
    } finally {
      setPulling(false);
    }
  };

  const repair = async (model: string) => {
    setPulling(true);
    setPullMsg(`${model} 재설치 중...`);
    try {
      const result = await invoke<string>('ollama_repair_model', { model });
      setPullMsg(result);
      await check();
    } catch (e: any) {
      setPullMsg(`실패: ${e}`);
    } finally {
      setPulling(false);
    }
  };

  if (!isTauri()) return null;

  return (
    <div style={{ marginTop: 24 }}>
      <h3 style={{ fontSize: 16, marginBottom: 12 }}>Ollama 관리</h3>
      {status === null ? (
        <p style={{ color: '#888' }}>확인 중...</p>
      ) : !status.installed ? (
        <p style={{ color: '#e94560' }}>Ollama가 설치되어 있지 않습니다. <a href="https://ollama.com/download" target="_blank" rel="noreferrer" style={{ color: '#4a9eff' }}>설치하기</a></p>
      ) : (
        <>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
            <span style={{ color: status.running ? '#4a9eff' : '#e94560' }}>
              {status.running ? '● 실행 중' : '● 중지됨'}
            </span>
            {!status.running && <button onClick={start} style={btn}>시작</button>}
            <button onClick={check} style={btn}>새로고침</button>
          </div>
          {status.models.length > 0 && (
            <div style={{ marginBottom: 12 }}>
              <p style={{ fontSize: 13, color: '#888', marginBottom: 6 }}>설치된 모델:</p>
              {status.models.map(m => (
                <div key={m} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '4px 0' }}>
                  <code style={{ fontSize: 13 }}>{m}</code>
                  <button onClick={() => repair(m)} disabled={pulling} style={{ ...btn, fontSize: 11 }}>재설치</button>
                </div>
              ))}
            </div>
          )}
          <div style={{ display: 'flex', gap: 6 }}>
            <input placeholder="모델명 (예: gemma3)" value={customModel} onChange={e => setCustomModel(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && customModel && pull(customModel)}
              style={{ flex: 1, padding: '6px 10px', background: '#0f0f1a', border: '1px solid #2a2a3e', borderRadius: 6, color: '#e0e0e0', fontSize: 13 }} />
            <button onClick={() => pull(customModel)} disabled={pulling || !customModel} style={btn}>다운로드</button>
          </div>
          {pullMsg && <p style={{ marginTop: 8, fontSize: 13, color: '#888' }}>{pullMsg}</p>}
        </>
      )}
    </div>
  );
}

const btn: React.CSSProperties = { background: '#2a2a3e', color: '#ccc', border: '1px solid #3a3a4e', borderRadius: 6, padding: '4px 12px', fontSize: 12, cursor: 'pointer' };
