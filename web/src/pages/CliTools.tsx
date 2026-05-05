import { useState, useEffect, useCallback } from 'react';
import { isTauri } from '../lib/tauri';

interface CliToolInfo {
  id: string;
  name: string;
  installed: boolean;
  version?: string;
}

const TOOL_META: Record<string, { description: string; icon: string; pkg: string }> = {
  claude: { description: '코드 리뷰, 리팩토링, 버그 수정', icon: 'C', pkg: '@anthropic-ai/claude-code' },
  codex:  { description: '코드 생성, 자동 완성',           icon: 'O', pkg: '@openai/codex' },
  gemini: { description: '멀티모달 분석, 이미지 이해',     icon: 'G', pkg: '@google/gemini-cli' },
  kiro:   { description: 'AI 개발 어시스턴트',             icon: 'K', pkg: '@aws/kiro-cli' },
};

const ICON_COLORS: Record<string, string> = {
  claude: 'var(--pc-accent)',
  codex:  'var(--pc-spring)',
  gemini: 'var(--pc-iris)',
  kiro:   'var(--pc-carp)',
};

const DEFAULT_TOOLS: CliToolInfo[] = [
  { id: 'claude', name: 'Claude Code', installed: false },
  { id: 'codex',  name: 'Codex CLI',   installed: false },
  { id: 'gemini', name: 'Gemini CLI',  installed: false },
  { id: 'kiro',   name: 'Kiro CLI',    installed: false },
];

type InstallState = 'idle' | 'installing' | 'error';

export default function CliTools() {
  const [tools, setTools] = useState<CliToolInfo[]>(DEFAULT_TOOLS);
  const [checking, setChecking] = useState(true);
  const [installState, setInstallState] = useState<Record<string, InstallState>>({});
  const [installError, setInstallError] = useState<Record<string, string>>({});

  const fetchTools = useCallback(async () => {
    setChecking(true);
    if (!isTauri()) { setChecking(false); return; }
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const list = await invoke<CliToolInfo[]>('list_cli_tools');
      setTools(DEFAULT_TOOLS.map(d => {
        const live = list.find(l => l.id === d.id);
        return live ? { ...d, installed: live.installed, version: live.version } : d;
      }));
    } catch {
      // 비 Tauri 환경 또는 오류 — 기본값 유지
    } finally {
      setChecking(false);
    }
  }, []);

  useEffect(() => { fetchTools(); }, [fetchTools]);

  const handleInstall = async (id: string) => {
    setInstallState(s => ({ ...s, [id]: 'installing' }));
    setInstallError(e => { const n = { ...e }; delete n[id]; return n; });
    try {
      if (isTauri()) {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('install_cli_tool', { tool: id });
        // 설치 후 상태 새로고침
        await fetchTools();
      } else {
        const pkg = TOOL_META[id]?.pkg ?? id;
        await navigator.clipboard.writeText(`npm install -g ${pkg}`);
        throw new Error('브라우저 환경 — 명령어가 클립보드에 복사되었습니다');
      }
      setInstallState(s => ({ ...s, [id]: 'idle' }));
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setInstallState(s => ({ ...s, [id]: 'error' }));
      setInstallError(err => ({ ...err, [id]: msg }));
    }
  };

  const installedCount = tools.filter(t => t.installed).length;

  return (
    <div style={{ padding: '0 24px 24px' }}>
      <div className="page-head">
        <div style={{ flex: 1 }}>
          <div className="crumb">시스템</div>
          <h1>AI CLI 도구</h1>
          <div className="sub">
            에이전트가 작업을 위임할 수 있는 외부 AI 도구
            {checking
              ? ' · 확인 중…'
              : ` · ${installedCount}/${tools.length} 설치됨`}
          </div>
        </div>
        <button
          onClick={fetchTools}
          disabled={checking}
          style={{ padding: '6px 14px', borderRadius: 8, border: '1px solid var(--pc-border)', background: 'transparent', color: 'var(--pc-text-muted)', cursor: checking ? 'default' : 'pointer', fontSize: 12.5 }}
        >
          {checking ? '확인 중…' : '새로고침'}
        </button>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 10, maxWidth: 620 }}>
        {tools.map(tool => {
          const meta = TOOL_META[tool.id];
          const color = ICON_COLORS[tool.id] ?? 'var(--pc-accent)';
          const state = installState[tool.id] ?? 'idle';
          const errMsg = installError[tool.id];

          return (
            <div
              key={tool.id}
              style={{
                display: 'flex', alignItems: 'center', gap: 14,
                padding: '14px 16px', borderRadius: 12,
                background: 'var(--pc-bg-elevated)',
                border: `1px solid ${tool.installed ? `${color}33` : 'var(--pc-border)'}`,
                transition: 'border-color 0.2s',
              }}
            >
              {/* 아이콘 */}
              <div style={{
                width: 40, height: 40, borderRadius: 11, flexShrink: 0,
                background: tool.installed ? `${color}22` : 'var(--pc-bg-input)',
                border: `1px solid ${tool.installed ? `${color}55` : 'var(--pc-border)'}`,
                display: 'flex', alignItems: 'center', justifyContent: 'center',
                fontWeight: 700, fontSize: 15,
                color: tool.installed ? color : 'var(--pc-text-muted)',
                transition: 'all 0.2s',
              }}>
                {meta?.icon ?? tool.id[0].toUpperCase()}
              </div>

              {/* 이름 + 설명 */}
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                  <span style={{ fontWeight: 600, fontSize: 13.5, color: 'var(--pc-text-primary)' }}>
                    {tool.name}
                  </span>
                  {checking ? (
                    <span style={{ fontSize: 11, color: 'var(--pc-text-muted)' }}>확인 중…</span>
                  ) : tool.installed ? (
                    <span style={{
                      fontSize: 11, fontWeight: 600, padding: '2px 7px', borderRadius: 20,
                      background: `${color}20`, color, border: `1px solid ${color}44`,
                    }}>
                      ✓ 설치됨{tool.version ? ` ${tool.version}` : ''}
                    </span>
                  ) : (
                    <span style={{
                      fontSize: 11, padding: '2px 7px', borderRadius: 20,
                      background: 'var(--pc-bg-input)', color: 'var(--pc-text-muted)',
                      border: '1px solid var(--pc-border)',
                    }}>
                      미설치
                    </span>
                  )}
                </div>
                <div style={{ fontSize: 12.5, color: 'var(--pc-text-muted)', marginTop: 3 }}>
                  {meta?.description}
                </div>
                {errMsg && (
                  <div style={{ fontSize: 11.5, color: 'var(--color-status-error)', marginTop: 4 }}>
                    {errMsg}
                  </div>
                )}
              </div>

              {/* 설치 버튼 — 미설치 시만 표시 */}
              {!checking && !tool.installed && (
                <button
                  onClick={() => handleInstall(tool.id)}
                  disabled={state === 'installing'}
                  style={{
                    padding: '7px 16px', borderRadius: 8, border: 'none',
                    cursor: state === 'installing' ? 'default' : 'pointer',
                    background: state === 'installing' ? 'var(--pc-bg-input)' : color,
                    color: state === 'installing' ? 'var(--pc-text-muted)' : '#1a1a22',
                    fontWeight: 600, fontSize: 12.5, flexShrink: 0,
                    display: 'flex', alignItems: 'center', gap: 6,
                    opacity: state === 'installing' ? 0.7 : 1,
                    transition: 'opacity 0.15s',
                  }}
                >
                  {state === 'installing' && (
                    <span className="narae-spinner" style={{ width: 13, height: 13, borderWidth: 1.5 }} />
                  )}
                  {state === 'installing' ? '설치 중…' : '설치'}
                </button>
              )}
            </div>
          );
        })}
      </div>

      {!isTauri() && (
        <p style={{ marginTop: 16, fontSize: 12, color: 'var(--pc-text-muted)', maxWidth: 620 }}>
          브라우저 환경에서는 설치 버튼을 누르면 npm 명령어가 클립보드에 복사됩니다.
        </p>
      )}
    </div>
  );
}
