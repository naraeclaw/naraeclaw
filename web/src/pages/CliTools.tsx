import { useState, useEffect } from 'react';
import { isTauri } from '../lib/tauri';

interface CliToolInfo {
  id: string;
  name: string;
  installed: boolean;
  install_hint: string;
  description: string;
}

const DEFAULT_TOOLS: CliToolInfo[] = [
  { id: 'claude', name: 'Claude Code', installed: false, install_hint: 'npm install -g @anthropic-ai/claude-code', description: '코드 리뷰, 리팩토링, 버그 수정' },
  { id: 'codex', name: 'Codex CLI', installed: false, install_hint: 'npm install -g @openai/codex', description: '코드 생성, 자동 완성' },
  { id: 'gemini', name: 'Gemini CLI', installed: false, install_hint: 'npm install -g @google/gemini-cli', description: '멀티모달 분석, 이미지 이해' },
  { id: 'kiro', name: 'Kiro CLI', installed: false, install_hint: 'npm install -g @aws/kiro-cli', description: 'AI 개발 어시스턴트' },
];

export default function CliTools() {
  const [tools, setTools] = useState<CliToolInfo[]>(DEFAULT_TOOLS);

  useEffect(() => {
    if (!isTauri()) return;
    (async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const list = await invoke<CliToolInfo[]>('list_cli_tools');
        // Merge with defaults for descriptions
        setTools(DEFAULT_TOOLS.map(d => {
          const live = list.find(l => l.id === d.id);
          return live ? { ...d, installed: live.installed } : d;
        }));
      } catch {}
    })();
  }, []);

  return (
    <div style={{ padding: 24, maxWidth: 600 }}>
      <h2 style={{ fontSize: 20, marginBottom: 4 }}>AI 도구</h2>
      <p style={{ color: '#888', fontSize: 14, marginBottom: 20 }}>에이전트가 작업을 위임할 수 있는 외부 AI CLI 도구</p>
      {tools.map(tool => (
        <div key={tool.id} style={cardStyle}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <div>
              <strong>{tool.name}</strong>
              <span style={{ marginLeft: 8, fontSize: 12, color: tool.installed ? '#4a9eff' : '#666' }}>
                {tool.installed ? '✓ 설치됨' : '미설치'}
              </span>
              <p style={{ color: '#888', fontSize: 13, margin: '4px 0 0' }}>{tool.description}</p>
            </div>
          </div>
          {!tool.installed && (
            <div style={{ marginTop: 8 }}>
              <p style={{ fontSize: 12, color: '#666', marginBottom: 4 }}>설치:</p>
              <code style={{ background: '#0f0f1a', padding: '6px 10px', borderRadius: 4, fontSize: 12, color: '#4a9eff', display: 'block' }}>
                {tool.install_hint}
              </code>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

const cardStyle: React.CSSProperties = { background: '#1a1a2e', padding: 16, borderRadius: 8, marginBottom: 8 };
