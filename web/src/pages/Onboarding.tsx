import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { isTauri } from '../lib/tauri';
import { invoke } from '@tauri-apps/api/core';

interface OllamaStatus {
  installed: boolean;
  running: boolean;
  models: string[];
}

type Step = 'welcome' | 'provider' | 'ollama' | 'apikey' | 'model' | 'done';

const PROVIDERS = [
  { id: 'ollama', name: 'Ollama', desc: '로컬 실행, 무료', badge: '추천' },
  { id: 'openrouter', name: 'OpenRouter', desc: '200+ 모델, API key 필요' },
  { id: 'anthropic', name: 'Anthropic', desc: 'Claude, API key 필요' },
  { id: 'openai', name: 'OpenAI', desc: 'GPT, API key 필요' },
];

const OLLAMA_MODELS = [
  { id: 'gemma3:latest', name: 'Gemma 3', desc: '가볍고 빠름', size: '5GB' },
  { id: 'llama3.2:latest', name: 'Llama 3.2', desc: '범용', size: '4GB' },
  { id: 'qwen2.5:latest', name: 'Qwen 2.5', desc: '다국어', size: '4GB' },
];

export default function Onboarding() {
  const navigate = useNavigate();
  const [step, setStep] = useState<Step>('welcome');
  const [provider, setProvider] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [model, setModel] = useState('');
  const [ollamaStatus, setOllamaStatus] = useState<OllamaStatus | null>(null);
  const [pulling, setPulling] = useState(false);
  const [pullMsg, setPullMsg] = useState('');
  const [error, setError] = useState('');

  const canInvoke = isTauri();

  useEffect(() => {
    if (!canInvoke) return;
    invoke('config_exists').then((exists) => {
      if (exists) navigate('/', { replace: true });
    }).catch(() => {});
  }, [canInvoke, navigate]);

  // Auto-check Ollama when entering the ollama step.
  useEffect(() => {
    if (step === 'ollama') {
      checkOllama();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step]);

  const checkOllama = async () => {
    try {
      const status = await invoke<OllamaStatus>('check_ollama');
      setOllamaStatus(status);
    } catch (e: any) {
      setError(`Ollama 확인 실패: ${e}`);
      setOllamaStatus({ installed: false, running: false, models: [] });
    }
  };

  const startOllama = async () => {
    try {
      await invoke('ollama_start');
      await checkOllama();
    } catch (e: any) {
      setError(e.toString());
    }
  };

  const pullModel = async (m: string) => {
    setPulling(true);
    setPullMsg(`${m} 다운로드 중...`);
    try {
      await invoke('ollama_pull', { model: m });
      setPullMsg(`${m} 완료!`);
      setModel(m);
      await checkOllama();
    } catch (e: any) {
      setPullMsg(`실패: ${e}`);
    } finally {
      setPulling(false);
    }
  };

  const finish = async () => {
    try {
      await invoke('complete_onboarding', {
        settings: { provider, model, api_key: apiKey || null },
      });
      setStep('done');
      // Gateway가 시작되면 메인 페이지로 이동. 약간의 대기 후 강제 이동.
      setTimeout(() => { window.location.pathname = '/'; }, 3000);
    } catch (e: any) {
      setError(e.toString());
    }
  };

  return (
    <div style={{
      minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center',
      background: '#0f0f1a', color: '#e0e0e0', fontFamily: '-apple-system, BlinkMacSystemFont, sans-serif',
    }}>
      <div style={{ maxWidth: 520, width: '100%', padding: 40 }}>

        {step === 'welcome' && (
          <div style={{ textAlign: 'center' }}>
            <div style={{ fontSize: 48, marginBottom: 16 }}>🦀</div>
            <h1 style={{ fontSize: 24, marginBottom: 8 }}>NaraeClaw</h1>
            <p style={{ color: '#888', marginBottom: 32 }}>서버 관리와 개인 지식을 위한 AI 에이전트</p>
            <button onClick={() => setStep('provider')} style={btnStyle}>
              시작하기
            </button>
          </div>
        )}

        {step === 'provider' && (
          <div>
            <h2 style={{ fontSize: 20, marginBottom: 20 }}>AI 엔진 선택</h2>
            {PROVIDERS.map(p => (
              <div key={p.id} onClick={() => {
                setProvider(p.id);
                if (p.id === 'ollama') { setStep('ollama'); checkOllama(); }
                else setStep('apikey');
              }} style={{
                ...cardStyle,
                border: provider === p.id ? '1px solid #4a9eff' : '1px solid #2a2a3e',
                cursor: 'pointer',
              }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <strong>{p.name}</strong>
                  {p.badge && <span style={{ background: '#4a9eff', color: '#fff', padding: '2px 8px', borderRadius: 4, fontSize: 12 }}>{p.badge}</span>}
                </div>
                <p style={{ color: '#888', margin: '4px 0 0', fontSize: 14 }}>{p.desc}</p>
              </div>
            ))}
          </div>
        )}

        {step === 'ollama' && (
          <div>
            <h2 style={{ fontSize: 20, marginBottom: 20 }}>Ollama 설정</h2>
            {ollamaStatus === null ? (
              <p style={{ color: '#888' }}>확인 중...</p>
            ) : !ollamaStatus.installed ? (
              <div style={cardStyle}>
                <p>Ollama가 설치되어 있지 않습니다.</p>
                <p style={{ color: '#888', fontSize: 14, marginTop: 8 }}>
                  <a href="https://ollama.com/download" target="_blank" rel="noreferrer"
                    style={{ color: '#4a9eff' }}>ollama.com</a>에서 설치하세요.
                </p>
                <button onClick={checkOllama} style={{ ...btnStyle, marginTop: 12 }}>다시 확인</button>
              </div>
            ) : !ollamaStatus.running ? (
              <div style={cardStyle}>
                <p>Ollama가 설치되었지만 실행 중이 아닙니다.</p>
                <button onClick={startOllama} style={{ ...btnStyle, marginTop: 12 }}>Ollama 시작</button>
              </div>
            ) : (
              <div>
                {ollamaStatus.models.length > 0 && (
                  <div style={{ marginBottom: 16 }}>
                    <p style={{ color: '#888', fontSize: 14, marginBottom: 8 }}>설치된 모델:</p>
                    {ollamaStatus.models.map(m => (
                      <div key={m} onClick={() => { setModel(m); }} style={{
                        ...cardStyle,
                        border: model === m ? '1px solid #4a9eff' : '1px solid #2a2a3e',
                        cursor: 'pointer', padding: '8px 16px',
                      }}>
                        {m}
                      </div>
                    ))}
                  </div>
                )}
                <p style={{ color: '#888', fontSize: 14, marginBottom: 8 }}>모델 다운로드:</p>
                {OLLAMA_MODELS.map(m => (
                  <div key={m.id} style={{ ...cardStyle, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                    <div>
                      <strong>{m.name}</strong>
                      <span style={{ color: '#888', fontSize: 12, marginLeft: 8 }}>{m.size}</span>
                      <p style={{ color: '#666', fontSize: 13, margin: '2px 0 0' }}>{m.desc}</p>
                    </div>
                    {ollamaStatus.models.some(installed => installed.startsWith(m.id.split(':')[0] ?? m.id)) ? (
                      <span style={{ color: '#4a9eff', fontSize: 13 }}>✓ 설치됨</span>
                    ) : (
                      <button onClick={() => pullModel(m.id)} disabled={pulling}
                        style={{ ...btnSmall, opacity: pulling ? 0.5 : 1 }}>
                        다운로드
                      </button>
                    )}
                  </div>
                ))}
                {pullMsg && <p style={{ color: '#888', fontSize: 13, marginTop: 8 }}>{pullMsg}</p>}
                {model && (
                  <button onClick={finish} style={{ ...btnStyle, marginTop: 16 }}>
                    완료 — {model} 사용
                  </button>
                )}
              </div>
            )}
            <button onClick={() => setStep('provider')} style={linkStyle}>← 뒤로</button>
          </div>
        )}

        {step === 'apikey' && (
          <div>
            <h2 style={{ fontSize: 20, marginBottom: 20 }}>
              {PROVIDERS.find(p => p.id === provider)?.name} API Key
            </h2>
            <input
              type="password"
              placeholder="sk-..."
              value={apiKey}
              onChange={e => setApiKey(e.target.value)}
              style={inputStyle}
            />
            <button onClick={() => setStep('model')} disabled={!apiKey}
              style={{ ...btnStyle, marginTop: 16, opacity: apiKey ? 1 : 0.5 }}>
              다음
            </button>
            <button onClick={() => setStep('provider')} style={linkStyle}>← 뒤로</button>
          </div>
        )}

        {step === 'model' && (
          <div>
            <h2 style={{ fontSize: 20, marginBottom: 20 }}>모델 선택</h2>
            <input
              type="text"
              placeholder="예: anthropic/claude-sonnet-4-20250514"
              value={model}
              onChange={e => setModel(e.target.value)}
              style={inputStyle}
            />
            <button onClick={finish} disabled={!model}
              style={{ ...btnStyle, marginTop: 16, opacity: model ? 1 : 0.5 }}>
              완료
            </button>
            <button onClick={() => setStep('apikey')} style={linkStyle}>← 뒤로</button>
          </div>
        )}

        {step === 'done' && (
          <div style={{ textAlign: 'center' }}>
            <div style={{ fontSize: 48, marginBottom: 16 }}>✅</div>
            <h2>설정 완료!</h2>
            <p style={{ color: '#888' }}>에이전트를 시작하는 중...</p>
          </div>
        )}

        {error && <p style={{ color: '#e94560', marginTop: 16, fontSize: 14 }}>{error}</p>}
      </div>
    </div>
  );
}

const btnStyle: React.CSSProperties = {
  background: '#4a9eff', color: '#fff', border: 'none', borderRadius: 8,
  padding: '12px 32px', fontSize: 16, cursor: 'pointer', width: '100%',
};

const btnSmall: React.CSSProperties = {
  background: '#2a2a3e', color: '#ccc', border: '1px solid #3a3a4e', borderRadius: 6,
  padding: '6px 14px', fontSize: 13, cursor: 'pointer',
};

const cardStyle: React.CSSProperties = {
  background: '#1a1a2e', padding: 16, borderRadius: 8, marginBottom: 8,
};

const inputStyle: React.CSSProperties = {
  width: '100%', padding: '12px 16px', background: '#1a1a2e', border: '1px solid #2a2a3e',
  borderRadius: 8, color: '#e0e0e0', fontSize: 15, outline: 'none', boxSizing: 'border-box',
};

const linkStyle: React.CSSProperties = {
  background: 'none', border: 'none', color: '#888', cursor: 'pointer',
  marginTop: 12, display: 'block', fontSize: 14,
};
