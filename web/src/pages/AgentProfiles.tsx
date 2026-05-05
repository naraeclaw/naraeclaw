import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { isTauri } from '@/lib/tauri';

interface ProfileMeta {
  id: string;
  name: string;
  provider: string;
  model: string;
  api_key_set: boolean;
  api_url: string | null;
  is_active: boolean;
  created_at: string;
  description: string | null;
}

interface CreateForm {
  id: string;
  name: string;
  provider: string;
  model: string;
  api_key: string;
  api_url: string;
  description: string;
}

const PROVIDERS = [
  { id: 'anthropic',  label: 'Anthropic',   hint: 'claude-haiku-4-5 / claude-sonnet-4-6', needsKey: true },
  { id: 'openrouter', label: 'OpenRouter',   hint: 'anthropic/claude-sonnet-4-6',         needsKey: true },
  { id: 'ollama',     label: 'Ollama',       hint: 'gemma3:latest / llama3.2:latest',      needsKey: false },
  { id: 'openai',     label: 'OpenAI',       hint: 'gpt-4o-mini / gpt-4o',                needsKey: true },
];

const PROVIDER_COLOR: Record<string, string> = {
  anthropic:  'var(--pc-sakura)',
  openrouter: 'var(--pc-iris)',
  ollama:     'var(--pc-spring)',
  openai:     'var(--pc-wave)',
};

function providerColor(p: string) {
  return PROVIDER_COLOR[p] ?? 'var(--pc-accent)';
}

function ProviderBadge({ provider }: { provider: string }) {
  const col = providerColor(provider);
  return (
    <span style={{
      display: 'inline-flex', alignItems: 'center',
      padding: '2px 8px', borderRadius: 99,
      fontSize: 10.5, fontWeight: 700, letterSpacing: '0.04em',
      background: `${col}18`, color: col,
      border: `1px solid ${col}33`,
      textTransform: 'capitalize' as const,
    }}>
      {provider}
    </span>
  );
}

function EmptyState({ onNew }: { onNew: () => void }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: 320, gap: 16, color: 'var(--pc-text-muted)' }}>
      <div style={{ width: 64, height: 64, borderRadius: 18, background: 'var(--pc-bg-elevated)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 28 }}>나</div>
      <div style={{ textAlign: 'center' }}>
        <div style={{ fontSize: 15, fontWeight: 600, color: 'var(--pc-text-primary)', marginBottom: 6 }}>에이전트 프로필이 없습니다</div>
        <div style={{ fontSize: 13 }}>나래 에이전트를 만들어 독립된 환경을 설정하세요</div>
      </div>
      <button className="btn primary" onClick={onNew}>+ 새 프로필 만들기</button>
    </div>
  );
}

function ProfileCard({ profile, onSwitch, onDelete, onRename, switching }: {
  profile: ProfileMeta;
  onSwitch: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string, name: string) => void;
  switching: string | null;
}) {
  const [editingName, setEditingName] = useState(false);
  const [nameVal, setNameVal] = useState(profile.name);
  const isSwitching = switching === profile.id;
  const col = providerColor(profile.provider);

  const saveName = () => {
    if (nameVal.trim() && nameVal !== profile.name) onRename(profile.id, nameVal.trim());
    setEditingName(false);
  };

  return (
    <div className="kw-card fade-in" style={{
      padding: 20,
      borderColor: profile.is_active ? `${col}50` : 'var(--pc-border)',
      background: profile.is_active ? `${col}08` : 'var(--pc-bg-surface)',
      position: 'relative',
      transition: 'all 0.2s',
    }}>
      {/* Active badge */}
      {profile.is_active && (
        <div style={{
          position: 'absolute', top: 14, right: 14,
          display: 'flex', alignItems: 'center', gap: 5,
          fontSize: 11, fontWeight: 700, color: col,
        }}>
          <span style={{ width: 6, height: 6, borderRadius: '50%', background: col, boxShadow: `0 0 6px ${col}`, display: 'inline-block' }} />
          활성
        </div>
      )}

      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'flex-start', gap: 12, marginBottom: 14 }}>
        <div style={{
          width: 42, height: 42, borderRadius: 12, flexShrink: 0,
          background: `${col}18`, color: col,
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          fontSize: 18, fontWeight: 700,
        }}>
          {profile.name[0]?.toUpperCase() ?? '?'}
        </div>
        <div style={{ flex: 1, minWidth: 0 }}>
          {editingName ? (
            <input
              autoFocus
              value={nameVal}
              onChange={e => setNameVal(e.target.value)}
              onBlur={saveName}
              onKeyDown={e => { if (e.key === 'Enter') saveName(); if (e.key === 'Escape') setEditingName(false); }}
              style={{ fontSize: 15, fontWeight: 700, color: 'var(--pc-text-primary)', background: 'var(--pc-bg-input)', border: '1px solid var(--pc-accent-dim)', borderRadius: 6, padding: '2px 8px', width: '100%' }}
            />
          ) : (
            <div
              style={{ fontSize: 15, fontWeight: 700, color: 'var(--pc-text-primary)', cursor: 'text', paddingBottom: 1 }}
              onClick={() => setEditingName(true)}
              title="클릭하여 이름 변경"
            >
              {profile.name}
            </div>
          )}
          {profile.description && (
            <div className="tiny" style={{ marginTop: 2 }}>{profile.description}</div>
          )}
        </div>
      </div>

      {/* Stats */}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, marginBottom: 16 }}>
        <div>
          <div className="tiny" style={{ marginBottom: 3 }}>공급자</div>
          <ProviderBadge provider={profile.provider} />
        </div>
        <div>
          <div className="tiny" style={{ marginBottom: 3 }}>모델</div>
          <div className="mono" style={{ fontSize: 11.5, color: 'var(--pc-text-secondary)' }}>{profile.model}</div>
        </div>
        <div>
          <div className="tiny" style={{ marginBottom: 3 }}>API 키</div>
          <div style={{ fontSize: 12, color: profile.api_key_set ? 'var(--color-status-success)' : 'var(--pc-text-faint)' }}>
            {profile.api_key_set ? '설정됨' : profile.provider === 'ollama' ? '불필요' : '미설정'}
          </div>
        </div>
        <div>
          <div className="tiny" style={{ marginBottom: 3 }}>ID</div>
          <div className="mono" style={{ fontSize: 11, color: 'var(--pc-text-faint)' }}>{profile.id}</div>
        </div>
      </div>

      {/* Actions */}
      <div style={{ display: 'flex', gap: 8, paddingTop: 14, borderTop: '1px solid var(--pc-separator)' }}>
        {!profile.is_active && (
          <button
            className="btn primary"
            onClick={() => onSwitch(profile.id)}
            disabled={isSwitching}
            style={{ flex: 1, justifyContent: 'center' }}
          >
            {isSwitching ? '전환 중…' : '이 프로필로 전환'}
          </button>
        )}
        {profile.is_active && (
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 13, color: col, fontWeight: 600 }}>
            현재 사용 중
          </div>
        )}
        <button
          className="btn ghost"
          onClick={() => { if (confirm(`'${profile.name}' 프로필을 삭제할까요?`)) onDelete(profile.id); }}
          disabled={profile.is_active}
          title={profile.is_active ? '활성 프로필은 삭제할 수 없습니다' : '삭제'}
          style={{ padding: '7px 10px', opacity: profile.is_active ? 0.3 : 1 }}
        >
          삭제
        </button>
      </div>
    </div>
  );
}

function CreateModal({ onClose, onCreate }: { onClose: () => void; onCreate: (f: CreateForm) => Promise<void> }) {
  const [form, setForm] = useState<CreateForm>({
    id: '', name: '', provider: 'anthropic', model: 'claude-haiku-4-5',
    api_key: '', api_url: '', description: '',
  });
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState('');

  const selectedProvider = PROVIDERS.find(p => p.id === form.provider);

  const set = (k: keyof CreateForm) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>) =>
    setForm(f => ({ ...f, [k]: e.target.value }));

  // Auto-generate ID from name
  const handleNameChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const name = e.target.value;
    const id = name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
    setForm(f => ({ ...f, name, id: f.id === autoId(f.name) ? id : f.id }));
  };

  function autoId(name: string) {
    return name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
  }

  const submit = async () => {
    if (!form.name.trim()) { setError('이름을 입력하세요'); return; }
    if (!form.id.trim()) { setError('ID를 입력하세요'); return; }
    if (!form.model.trim()) { setError('모델을 입력하세요'); return; }
    setSubmitting(true);
    setError('');
    try {
      await onCreate(form);
      onClose();
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div style={{
      position: 'fixed', inset: 0, zIndex: 200,
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      background: 'rgba(15,15,20,0.75)', backdropFilter: 'blur(6px)',
    }} onClick={e => { if (e.target === e.currentTarget) onClose(); }}>
      <div style={{
        width: 500, background: 'var(--pc-bg-surface)',
        border: '1px solid var(--pc-border-strong)',
        borderRadius: 18, padding: 28,
        boxShadow: '0 24px 64px rgba(0,0,0,0.5)',
      }}>
        <div style={{ marginBottom: 22 }}>
          <div style={{ fontSize: 17, fontWeight: 700, color: 'var(--pc-text-primary)' }}>새 에이전트 프로필</div>
          <div className="tiny" style={{ marginTop: 4 }}>독립된 공급자·모델·API 키 조합으로 에이전트를 구성합니다</div>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          {/* Name */}
          <div>
            <label className="tiny" style={{ display: 'block', marginBottom: 5 }}>프로필 이름 *</label>
            <input
              className="input-electric"
              placeholder="예: 업무용 Claude, 로컬 Ollama"
              value={form.name}
              onChange={handleNameChange}
              style={{ width: '100%' }}
            />
          </div>

          {/* ID */}
          <div>
            <label className="tiny" style={{ display: 'block', marginBottom: 5 }}>ID (파일명, 영문·숫자·하이픈) *</label>
            <input
              className="input-electric mono"
              placeholder="work-claude"
              value={form.id}
              onChange={set('id')}
              style={{ width: '100%', fontFamily: 'var(--pc-font-mono)', fontSize: 13 }}
            />
          </div>

          {/* Provider */}
          <div>
            <label className="tiny" style={{ display: 'block', marginBottom: 5 }}>공급자 *</label>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 8 }}>
              {PROVIDERS.map(p => (
                <button
                  key={p.id}
                  onClick={() => setForm(f => ({ ...f, provider: p.id, model: p.hint.split(' / ')[0] ?? '' }))}
                  style={{
                    padding: '10px 14px', borderRadius: 10, border: '1px solid',
                    cursor: 'pointer', textAlign: 'left' as const, transition: 'all 0.15s',
                    borderColor: form.provider === p.id ? providerColor(p.id) : 'var(--pc-border)',
                    background: form.provider === p.id ? `${providerColor(p.id)}12` : 'var(--pc-bg-elevated)',
                  }}
                >
                  <div style={{ fontSize: 13, fontWeight: 600, color: form.provider === p.id ? providerColor(p.id) : 'var(--pc-text-primary)' }}>{p.label}</div>
                  <div className="mono tiny" style={{ marginTop: 2 }}>{p.hint.split(' / ')[0]}</div>
                </button>
              ))}
            </div>
          </div>

          {/* Model */}
          <div>
            <label className="tiny" style={{ display: 'block', marginBottom: 5 }}>모델 *</label>
            <input
              className="input-electric mono"
              placeholder={selectedProvider?.hint ?? ''}
              value={form.model}
              onChange={set('model')}
              style={{ width: '100%', fontFamily: 'var(--pc-font-mono)', fontSize: 13 }}
            />
            {selectedProvider && (
              <div className="tiny" style={{ marginTop: 4 }}>예: {selectedProvider.hint}</div>
            )}
          </div>

          {/* API Key */}
          {selectedProvider?.needsKey && (
            <div>
              <label className="tiny" style={{ display: 'block', marginBottom: 5 }}>API 키</label>
              <input
                className="input-electric"
                type="password"
                placeholder="sk-..."
                value={form.api_key}
                onChange={set('api_key')}
                style={{ width: '100%', fontFamily: 'var(--pc-font-mono)' }}
              />
            </div>
          )}

          {/* API URL override */}
          {form.provider === 'ollama' && (
            <div>
              <label className="tiny" style={{ display: 'block', marginBottom: 5 }}>Ollama URL (선택)</label>
              <input
                className="input-electric"
                placeholder="http://127.0.0.1:11434"
                value={form.api_url}
                onChange={set('api_url')}
                style={{ width: '100%', fontFamily: 'var(--pc-font-mono)', fontSize: 13 }}
              />
            </div>
          )}

          {/* Description */}
          <div>
            <label className="tiny" style={{ display: 'block', marginBottom: 5 }}>설명 (선택)</label>
            <input
              className="input-electric"
              placeholder="이 프로필의 용도..."
              value={form.description}
              onChange={set('description')}
              style={{ width: '100%' }}
            />
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
            {submitting ? '생성 중…' : '프로필 만들기'}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Web fallback when not in Tauri ────────────────────────────────────────────

const LS_KEY = 'naraeclaw_profiles';

function loadLocalProfiles(): ProfileMeta[] {
  try {
    return JSON.parse(localStorage.getItem(LS_KEY) ?? '[]');
  } catch { return []; }
}
function saveLocalProfiles(p: ProfileMeta[]) {
  localStorage.setItem(LS_KEY, JSON.stringify(p));
}

// ── Main page ─────────────────────────────────────────────────────────────────

export default function AgentProfiles() {
  const [profiles, setProfiles] = useState<ProfileMeta[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [switching, setSwitching] = useState<string | null>(null);
  const inTauri = isTauri();

  const load = async () => {
    try {
      const data = inTauri
        ? await invoke<ProfileMeta[]>('list_profiles')
        : loadLocalProfiles();
      setProfiles(data);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { load(); }, []);

  const handleCreate = async (form: CreateForm) => {
    if (inTauri) {
      const created = await invoke<ProfileMeta>('create_profile', {
        req: {
          id: form.id,
          name: form.name,
          provider: form.provider,
          model: form.model,
          api_key: form.api_key || null,
          api_url: form.api_url || null,
          description: form.description || null,
        },
      });
      setProfiles(prev => [...prev, created]);
    } else {
      const now = new Date().toISOString();
      const created: ProfileMeta = {
        id: form.id,
        name: form.name,
        provider: form.provider,
        model: form.model,
        api_key_set: !!form.api_key,
        api_url: form.api_url || null,
        is_active: profiles.length === 0,
        created_at: now,
        description: form.description || null,
      };
      const next = [...profiles, created];
      saveLocalProfiles(next);
      setProfiles(next);
    }
  };

  const handleSwitch = async (id: string) => {
    setSwitching(id);
    try {
      if (inTauri) {
        await invoke('switch_profile', { id });
      } else {
        const next = profiles.map(p => ({ ...p, is_active: p.id === id }));
        saveLocalProfiles(next);
        setProfiles(next);
      }
      await load();
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setSwitching(null);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      if (inTauri) {
        await invoke('delete_profile', { id });
      } else {
        const next = profiles.filter(p => p.id !== id);
        saveLocalProfiles(next);
        setProfiles(next);
        return;
      }
      await load();
    } catch (e: unknown) {
      setError(String(e));
    }
  };

  const handleRename = async (id: string, name: string) => {
    try {
      if (inTauri) {
        await invoke('update_profile_meta', { id, name, description: profiles.find(p => p.id === id)?.description ?? null });
      } else {
        const next = profiles.map(p => p.id === id ? { ...p, name } : p);
        saveLocalProfiles(next);
        setProfiles(next);
        return;
      }
      await load();
    } catch (e: unknown) {
      setError(String(e));
    }
  };

  const activeProfile = profiles.find(p => p.is_active);

  return (
    <div style={{ flex: 1, overflowY: 'auto' }}>
      <div className="page-head">
        <div style={{ flex: 1 }}>
          <div className="crumb">에이전트</div>
          <h1>프로필 관리</h1>
          <div className="sub">
            {profiles.length > 0
              ? `${profiles.length}개 프로필 · 활성: ${activeProfile?.name ?? '없음'}`
              : '에이전트 프로필을 만들어 독립된 환경을 구성하세요'}
          </div>
        </div>
        {profiles.length > 0 && (
          <button className="btn primary" onClick={() => setShowCreate(true)}>
            + 새 프로필
          </button>
        )}
      </div>

      <div style={{ padding: '4px 32px 48px' }}>
        {/* Error banner */}
        {error && (
          <div style={{ marginBottom: 16, padding: '10px 14px', borderRadius: 10, background: 'rgba(195,64,67,0.1)', border: '1px solid rgba(195,64,67,0.25)', color: 'var(--color-status-error)', fontSize: 13, display: 'flex', alignItems: 'center', gap: 10 }}>
            <span style={{ flex: 1 }}>{error}</span>
            <button onClick={() => setError(null)} style={{ background: 'none', border: 'none', color: 'inherit', cursor: 'pointer', fontSize: 16 }}>×</button>
          </div>
        )}

        {/* Web-mode notice */}
        {!inTauri && (
          <div style={{ marginBottom: 16, padding: '10px 14px', borderRadius: 10, background: 'rgba(126,156,216,0.08)', border: '1px solid rgba(126,156,216,0.2)', color: 'var(--pc-text-secondary)', fontSize: 12.5 }}>
            브라우저 모드 — 프로필이 localStorage에 저장됩니다. 데스크탑 앱에서 실행하면 파일로 영구 저장됩니다.
          </div>
        )}

        {loading ? (
          <div style={{ display: 'flex', justifyContent: 'center', paddingTop: 80 }}>
            <div className="narae-spinner" />
          </div>
        ) : profiles.length === 0 ? (
          <EmptyState onNew={() => setShowCreate(true)} />
        ) : (
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(300px, 1fr))', gap: 14 }}>
            {profiles.map(p => (
              <ProfileCard
                key={p.id}
                profile={p}
                onSwitch={handleSwitch}
                onDelete={handleDelete}
                onRename={handleRename}
                switching={switching}
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
