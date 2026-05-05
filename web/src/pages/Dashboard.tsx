import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { listAgents, createAgent, deleteAgent, type Agent, type AgentColor, COLOR_VARS, COLOR_LABELS } from '@/lib/agentStore';

// ─── 헬퍼 ────────────────────────────────────────────────────────────────────

function formatRelative(iso?: string): string {
  if (!iso) return '';
  try {
    const diff = Date.now() - new Date(iso).getTime();
    const s = Math.floor(diff / 1000);
    if (s < 60) return '방금';
    const m = Math.floor(s / 60);
    if (m < 60) return `${m}분 전`;
    const h = Math.floor(m / 60);
    if (h < 24) return `${h}시간 전`;
    return `${Math.floor(h / 24)}일 전`;
  } catch { return ''; }
}

function autoSlug(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9가-힣]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 32) || 'agent';
}

const PROVIDERS = [
  { id: 'anthropic',  label: 'Anthropic',  models: ['claude-haiku-4-5', 'claude-sonnet-4-6'] },
  { id: 'openrouter', label: 'OpenRouter',  models: ['anthropic/claude-sonnet-4-6', 'openai/gpt-4o-mini'] },
  { id: 'ollama',     label: 'Ollama',      models: ['gemma3:latest', 'llama3.2:latest', 'qwen2.5:latest'] },
  { id: 'openai',     label: 'OpenAI',      models: ['gpt-4o-mini', 'gpt-4o'] },
];

const COLORS: AgentColor[] = ['accent', 'iris', 'spring', 'sakura', 'carp', 'wave'];

// ─── 에이전트 생성 모달 ───────────────────────────────────────────────────────

interface CreateModalProps {
  onClose: () => void;
  onCreate: (agent: Agent) => void;
}

function CreateModal({ onClose, onCreate }: CreateModalProps) {
  const [name, setName] = useState('');
  const [id, setId] = useState('');
  const [idTouched, setIdTouched] = useState(false);
  const [provider, setProvider] = useState('anthropic');
  const [model, setModel] = useState('claude-haiku-4-5');
  const [color, setColor] = useState<AgentColor>('accent');
  const [description, setDescription] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const selectedProvider = PROVIDERS.find(p => p.id === provider)!;

  const handleNameChange = (v: string) => {
    setName(v);
    if (!idTouched) setId(autoSlug(v));
  };

  const handleProviderChange = (pid: string) => {
    setProvider(pid);
    const p = PROVIDERS.find(x => x.id === pid);
    if (p) setModel(p.models[0] ?? '');
  };

  const submit = async () => {
    if (!name.trim()) { setError('이름을 입력하세요'); return; }
    if (!id.trim()) { setError('ID를 입력하세요'); return; }
    if (!model.trim()) { setError('모델을 입력하세요'); return; }
    setSubmitting(true); setError('');
    try {
      const agent = createAgent({ id, name, provider, model, color, description: description || undefined });
      onCreate(agent);
      onClose();
    } catch (e: unknown) {
      setError(String(e));
      setSubmitting(false);
    }
  };

  return (
    <div
      style={{ position: 'fixed', inset: 0, zIndex: 200, display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'rgba(15,15,20,0.8)', backdropFilter: 'blur(8px)' }}
      onClick={e => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div style={{ width: 480, background: 'var(--pc-bg-surface)', border: '1px solid var(--pc-border-strong)', borderRadius: 20, padding: 28, boxShadow: '0 32px 80px rgba(0,0,0,0.55)' }}>
        <div style={{ marginBottom: 22 }}>
          <div style={{ fontSize: 17, fontWeight: 700, color: 'var(--pc-text-primary)' }}>새 에이전트 만들기</div>
          <div className="tiny" style={{ marginTop: 4 }}>이름, 모델, 개성을 설정하면 독립된 대화 공간이 생성됩니다</div>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
          {/* 이름 */}
          <div>
            <label className="tiny" style={{ display: 'block', marginBottom: 5 }}>에이전트 이름 *</label>
            <input className="input-electric" placeholder="예: 코딩 도우미, 업무용 Claude" value={name}
              onChange={e => handleNameChange(e.target.value)} style={{ width: '100%' }} autoFocus />
          </div>

          {/* 색상 */}
          <div>
            <label className="tiny" style={{ display: 'block', marginBottom: 8 }}>아바타 색상</label>
            <div style={{ display: 'flex', gap: 10 }}>
              {COLORS.map(c => (
                <button key={c} onClick={() => setColor(c)} title={COLOR_LABELS[c]} style={{
                  width: 28, height: 28, borderRadius: '50%', border: color === c ? '2px solid var(--pc-text-primary)' : '2px solid transparent',
                  background: COLOR_VARS[c], cursor: 'pointer', outline: color === c ? `3px solid ${COLOR_VARS[c]}` : 'none', outlineOffset: 2,
                }} />
              ))}
            </div>
          </div>

          {/* 공급자 */}
          <div>
            <label className="tiny" style={{ display: 'block', marginBottom: 8 }}>공급자 *</label>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 8 }}>
              {PROVIDERS.map(p => (
                <button key={p.id} onClick={() => handleProviderChange(p.id)} style={{
                  padding: '9px 14px', borderRadius: 10, border: '1px solid',
                  borderColor: provider === p.id ? COLOR_VARS.accent : 'var(--pc-border)',
                  background: provider === p.id ? 'rgba(126,156,216,0.10)' : 'var(--pc-bg-elevated)',
                  cursor: 'pointer', textAlign: 'left' as const,
                }}>
                  <div style={{ fontSize: 13, fontWeight: 600, color: provider === p.id ? 'var(--pc-accent)' : 'var(--pc-text-primary)' }}>{p.label}</div>
                </button>
              ))}
            </div>
          </div>

          {/* 모델 */}
          <div>
            <label className="tiny" style={{ display: 'block', marginBottom: 5 }}>모델 *</label>
            <input className="input-electric" placeholder={selectedProvider.models[0]} value={model}
              onChange={e => setModel(e.target.value)}
              style={{ width: '100%', fontFamily: 'var(--pc-font-mono)', fontSize: 12.5 }} />
            <div style={{ display: 'flex', gap: 6, marginTop: 6, flexWrap: 'wrap' as const }}>
              {selectedProvider.models.map(m => (
                <button key={m} onClick={() => setModel(m)} style={{
                  padding: '2px 8px', borderRadius: 6, border: '1px solid',
                  borderColor: model === m ? 'var(--pc-accent)' : 'var(--pc-border)',
                  background: model === m ? 'rgba(126,156,216,0.1)' : 'transparent',
                  color: model === m ? 'var(--pc-accent)' : 'var(--pc-text-muted)',
                  fontSize: 10.5, cursor: 'pointer', fontFamily: 'var(--pc-font-mono)',
                }}>
                  {m}
                </button>
              ))}
            </div>
          </div>

          {/* ID & 설명 */}
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
            <div>
              <label className="tiny" style={{ display: 'block', marginBottom: 5 }}>ID (영문·숫자·하이픈)</label>
              <input className="input-electric" value={id}
                onChange={e => { setId(e.target.value); setIdTouched(true); }}
                style={{ width: '100%', fontFamily: 'var(--pc-font-mono)', fontSize: 12 }} />
            </div>
            <div>
              <label className="tiny" style={{ display: 'block', marginBottom: 5 }}>한 줄 설명 (선택)</label>
              <input className="input-electric" placeholder="이 에이전트의 역할…" value={description}
                onChange={e => setDescription(e.target.value)} style={{ width: '100%' }} />
            </div>
          </div>
        </div>

        {error && (
          <div style={{ marginTop: 14, padding: '8px 12px', borderRadius: 8, background: 'rgba(195,64,67,0.1)', border: '1px solid rgba(195,64,67,0.25)', color: 'var(--color-status-error)', fontSize: 12.5 }}>
            {error}
          </div>
        )}

        <div style={{ display: 'flex', gap: 10, marginTop: 22, justifyContent: 'flex-end' }}>
          <button className="btn ghost" onClick={onClose}>취소</button>
          <button className="btn primary" onClick={submit} disabled={submitting}>
            {submitting ? '생성 중…' : '에이전트 만들기'}
          </button>
        </div>
      </div>
    </div>
  );
}

// ─── 에이전트 카드 ─────────────────────────────────────────────────────────────

function AgentCard({ agent, onClick, onSettings, onDelete }: { agent: Agent; onClick: () => void; onSettings: () => void; onDelete: () => void }) {
  const col = COLOR_VARS[agent.color];
  const [hover, setHover] = useState(false);

  return (
    <div
      className="kw-card fade-in"
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        padding: 20, cursor: 'pointer', transition: 'all 0.18s',
        borderColor: hover ? `${col}60` : 'var(--pc-border)',
        background: hover ? `${col}08` : 'var(--pc-bg-surface)',
        transform: hover ? 'translateY(-2px)' : 'none',
        boxShadow: hover ? `0 8px 24px ${col}18` : 'none',
      }}
    >
      {/* Avatar + header */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 14, marginBottom: 14 }}>
        <div style={{
          width: 48, height: 48, borderRadius: 14, flexShrink: 0,
          background: `${col}20`, color: col,
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          fontSize: 22, fontWeight: 700,
          boxShadow: hover ? `0 0 16px ${col}30` : 'none',
          transition: 'box-shadow 0.18s',
        }}>
          {agent.name[0]?.toUpperCase() ?? 'A'}
        </div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 15, fontWeight: 700, color: 'var(--pc-text-primary)', marginBottom: 2 }}>{agent.name}</div>
          {agent.description && <div className="tiny" style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{agent.description}</div>}
        </div>
        {/* 호버 시 액션 버튼들 */}
        <div style={{ display: 'flex', gap: 4, opacity: hover ? 1 : 0, transition: 'opacity 0.15s' }}>
          <button
            onClick={e => { e.stopPropagation(); onSettings(); }}
            title="설정"
            style={{ padding: '4px 8px', borderRadius: 7, border: '1px solid var(--pc-border)', background: 'var(--pc-bg-input)', color: 'var(--pc-text-muted)', cursor: 'pointer', fontSize: 13 }}
          >
            ⚙
          </button>
          <button
            onClick={e => { e.stopPropagation(); if (confirm(`'${agent.name}'을 삭제할까요?\n대화 기록도 모두 사라집니다.`)) onDelete(); }}
            title="삭제"
            style={{ padding: '4px 8px', borderRadius: 7, border: '1px solid var(--pc-border)', background: 'var(--pc-bg-input)', color: 'var(--pc-text-muted)', cursor: 'pointer', fontSize: 12 }}
          >
            ✕
          </button>
        </div>
      </div>

      {/* Stats */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, paddingTop: 12, borderTop: '1px solid var(--pc-separator)' }}>
        <div>
          <div className="tiny" style={{ marginBottom: 3 }}>모델</div>
          <div className="mono" style={{ fontSize: 11, color: 'var(--pc-text-secondary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{agent.model}</div>
        </div>
        <div>
          <div className="tiny" style={{ marginBottom: 3 }}>메시지</div>
          <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--pc-text-primary)' }}>
            {agent.messageCount > 0 ? agent.messageCount : '—'}
          </div>
        </div>
        <div>
          <div className="tiny" style={{ marginBottom: 3 }}>공급자</div>
          <div style={{ fontSize: 11.5, color: col, fontWeight: 600, textTransform: 'capitalize' as const }}>{agent.provider}</div>
        </div>
        <div>
          <div className="tiny" style={{ marginBottom: 3 }}>마지막 대화</div>
          <div style={{ fontSize: 12, color: 'var(--pc-text-muted)' }}>{formatRelative(agent.lastMessageAt) || '없음'}</div>
        </div>
      </div>

      {/* CTA */}
      <div style={{
        marginTop: 14, padding: '8px 0 0', display: 'flex', alignItems: 'center', justifyContent: 'center',
        color: col, fontSize: 12.5, fontWeight: 600, opacity: hover ? 1 : 0,
        transition: 'opacity 0.18s',
      }}>
        대화 시작하기 →
      </div>
    </div>
  );
}

// ─── 빈 상태 ──────────────────────────────────────────────────────────────────

function EmptyState({ onNew }: { onNew: () => void }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', minHeight: 360, gap: 20, color: 'var(--pc-text-muted)' }}>
      {/* Decorative marks */}
      <div style={{ display: 'flex', gap: 16 }}>
        {(['accent', 'iris', 'spring'] as AgentColor[]).map((c, i) => (
          <div key={c} style={{
            width: 56, height: 56, borderRadius: 16,
            background: `${COLOR_VARS[c]}18`, border: `1px solid ${COLOR_VARS[c]}30`,
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            fontSize: 22, fontWeight: 700, color: COLOR_VARS[c],
            transform: i === 1 ? 'translateY(-8px)' : 'none',
            opacity: i === 1 ? 1 : 0.5,
          }}>
            {i === 0 ? 'A' : i === 1 ? '나' : 'B'}
          </div>
        ))}
      </div>
      <div style={{ textAlign: 'center' }}>
        <div style={{ fontSize: 18, fontWeight: 700, color: 'var(--pc-text-primary)', marginBottom: 8 }}>에이전트가 없습니다</div>
        <div style={{ fontSize: 13.5, lineHeight: 1.6 }}>
          에이전트를 만들면 각자 독립된 대화 공간이 생깁니다.<br />
          업무용, 코딩용, 한국어 전용 등 목적에 맞게 구성하세요.
        </div>
      </div>
      <button className="btn primary" onClick={onNew} style={{ fontSize: 14, padding: '10px 22px' }}>
        + 첫 에이전트 만들기
      </button>
    </div>
  );
}

// ─── 메인 ─────────────────────────────────────────────────────────────────────

export default function Dashboard() {
  const navigate = useNavigate();
  const [agents, setAgents] = useState<Agent[]>([]);
  const [showCreate, setShowCreate] = useState(false);

  useEffect(() => { setAgents(listAgents()); }, []);

  const handleCreate = (agent: Agent) => {
    setAgents(listAgents());
    navigate(`/chat/${agent.id}`);
  };

  const handleDelete = (id: string) => {
    deleteAgent(id);
    setAgents(listAgents());
  };

  return (
    <div style={{ flex: 1, overflowY: 'auto' }}>
      <div className="page-head">
        <div style={{ flex: 1 }}>
          <div className="crumb">나래클로</div>
          <h1>에이전트</h1>
          <div className="sub">
            {agents.length > 0
              ? `${agents.length}개의 에이전트 · 각자 독립된 대화 공간을 가집니다`
              : '첫 에이전트를 만들어보세요'}
          </div>
        </div>
        {agents.length > 0 && (
          <button className="btn primary" onClick={() => setShowCreate(true)}>
            + 새 에이전트
          </button>
        )}
      </div>

      <div style={{ padding: '4px 32px 48px' }}>
        {agents.length === 0 ? (
          <EmptyState onNew={() => setShowCreate(true)} />
        ) : (
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: 14 }}>
            {agents.map(a => (
              <AgentCard
                key={a.id}
                agent={a}
                onClick={() => navigate(`/chat/${a.id}`)}
                onSettings={() => navigate(`/agent/${a.id}/settings`)}
                onDelete={() => handleDelete(a.id)}
              />
            ))}
          </div>
        )}
      </div>

      {showCreate && (
        <CreateModal
          onClose={() => setShowCreate(false)}
          onCreate={handleCreate}
        />
      )}
    </div>
  );
}
