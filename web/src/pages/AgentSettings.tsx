import { useState, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import {
  getAgent, updateAgent, updateAgentPolicy, deleteAgent,
  defaultPolicy, COLOR_VARS, COLOR_LABELS,
  type AgentColor, type AgentPolicy,
} from '../lib/agentStore';

// ── 태그 입력 컴포넌트 ────────────────────────────────────────────
function TagInput({
  label, hint, tags, onChange, placeholder = '추가 후 Enter',
}: {
  label: string; hint?: string; tags: string[]; onChange: (t: string[]) => void; placeholder?: string;
}) {
  const [input, setInput] = useState('');

  const add = (raw: string) => {
    const v = raw.trim();
    if (v && !tags.includes(v)) onChange([...tags, v]);
    setInput('');
  };

  const remove = (i: number) => onChange(tags.filter((_, idx) => idx !== i));

  return (
    <div>
      <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--pc-text-muted)', marginBottom: 6, textTransform: 'uppercase', letterSpacing: '0.04em' }}>{label}</div>
      {hint && <div style={{ fontSize: 12, color: 'var(--pc-text-muted)', marginBottom: 8 }}>{hint}</div>}
      <div style={{
        minHeight: 44, padding: '6px 8px', borderRadius: 9, border: '1px solid var(--pc-border)',
        background: 'var(--pc-bg-input)', display: 'flex', flexWrap: 'wrap', gap: 6, alignItems: 'center',
      }}>
        {tags.map((t, i) => (
          <span key={i} style={{
            display: 'inline-flex', alignItems: 'center', gap: 4,
            padding: '3px 8px', borderRadius: 20,
            background: 'var(--pc-bg-elevated)', border: '1px solid var(--pc-border)',
            fontSize: 12, color: 'var(--pc-text-secondary)',
          }}>
            {t}
            <button
              onClick={() => remove(i)}
              style={{ border: 'none', background: 'none', cursor: 'pointer', color: 'var(--pc-text-muted)', padding: 0, lineHeight: 1, fontSize: 13 }}
            >×</button>
          </span>
        ))}
        <input
          value={input}
          onChange={e => setInput(e.target.value)}
          onKeyDown={e => {
            if (e.key === 'Enter') { e.preventDefault(); add(input); }
            if (e.key === 'Backspace' && !input && tags.length) remove(tags.length - 1);
          }}
          onBlur={() => { if (input.trim()) add(input); }}
          placeholder={tags.length === 0 ? placeholder : ''}
          style={{ flex: 1, minWidth: 120, border: 'none', outline: 'none', background: 'transparent', fontSize: 13, color: 'var(--pc-text-primary)' }}
        />
      </div>
    </div>
  );
}

// ── 섹션 헤더 ─────────────────────────────────────────────────────
function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: 28 }}>
      <div style={{ fontSize: 11, fontWeight: 700, color: 'var(--pc-text-muted)', textTransform: 'uppercase', letterSpacing: '0.07em', marginBottom: 14, paddingBottom: 8, borderBottom: '1px solid var(--pc-border)' }}>
        {title}
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
        {children}
      </div>
    </div>
  );
}

// ── 레이블 + 입력 래퍼 ───────────────────────────────────────────
function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div>
      <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--pc-text-muted)', marginBottom: 6, textTransform: 'uppercase', letterSpacing: '0.04em' }}>{label}</div>
      {hint && <div style={{ fontSize: 12, color: 'var(--pc-text-muted)', marginBottom: 6 }}>{hint}</div>}
      {children}
    </div>
  );
}

const inputStyle: React.CSSProperties = {
  width: '100%', padding: '9px 12px', borderRadius: 9, border: '1px solid var(--pc-border)',
  background: 'var(--pc-bg-input)', color: 'var(--pc-text-primary)', fontSize: 13,
  outline: 'none', boxSizing: 'border-box',
};

const PROVIDERS = ['Claude', 'OpenAI', 'Ollama', 'Gemini'];
const MODELS: Record<string, string[]> = {
  Claude: ['claude-sonnet-4-6', 'claude-opus-4-7', 'claude-haiku-4-5'],
  OpenAI: ['gpt-4o', 'gpt-4o-mini', 'o3-mini'],
  Ollama: ['llama3.3', 'qwen2.5', 'mistral'],
  Gemini: ['gemini-2.0-flash', 'gemini-2.5-pro'],
};
const COLORS: AgentColor[] = ['accent', 'iris', 'spring', 'sakura', 'carp', 'wave'];

export default function AgentSettings() {
  const { agentId } = useParams<{ agentId: string }>();
  const navigate = useNavigate();
  const agent = agentId ? getAgent(agentId) : undefined;

  const [name, setName] = useState(agent?.name ?? '');
  const [description, setDescription] = useState(agent?.description ?? '');
  const [color, setColor] = useState<AgentColor>(agent?.color ?? 'accent');
  const [provider, setProvider] = useState(agent?.provider ?? 'Claude');
  const [model, setModel] = useState(agent?.model ?? '');
  const [policy, setPolicy] = useState<AgentPolicy>(agent?.policy ?? defaultPolicy());
  const [saved, setSaved] = useState(false);
  const [showDelete, setShowDelete] = useState(false);

  useEffect(() => {
    if (!agent) navigate('/', { replace: true });
  }, [agent, navigate]);

  if (!agent) return null;

  const policySet = <K extends keyof AgentPolicy>(key: K, val: AgentPolicy[K]) =>
    setPolicy(p => ({ ...p, [key]: val }));

  const handleSave = () => {
    updateAgent(agent.id, { name, description, color, provider, model });
    updateAgentPolicy(agent.id, policy);
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const handleDelete = () => {
    deleteAgent(agent.id);
    navigate('/', { replace: true });
  };

  const agentColor = COLOR_VARS[color];

  return (
    <div style={{ padding: '0 24px 40px' }}>
      {/* 헤더 */}
      <div className="page-head">
        <div style={{ flex: 1 }}>
          <div className="crumb">
            <button
              onClick={() => navigate('/')}
              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--pc-text-muted)', fontSize: 11.5, padding: 0 }}
            >
              ← 에이전트 목록
            </button>
            <span style={{ color: 'var(--pc-text-muted)', margin: '0 6px' }}>/</span>
            <button
              onClick={() => navigate(`/chat/${agent.id}`)}
              style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--pc-text-muted)', fontSize: 11.5, padding: 0 }}
            >
              {agent.name}
            </button>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 4 }}>
            <div style={{
              width: 32, height: 32, borderRadius: 10, flexShrink: 0,
              background: `linear-gradient(135deg, ${agentColor}, ${agentColor}88)`,
              display: 'flex', alignItems: 'center', justifyContent: 'center',
              fontWeight: 700, fontSize: 14, color: '#1a1a22',
            }}>{name.charAt(0).toUpperCase() || '?'}</div>
            <h1 style={{ margin: 0 }}>에이전트 설정</h1>
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <button
            onClick={() => navigate(`/chat/${agent.id}`)}
            style={{ padding: '7px 14px', borderRadius: 8, border: '1px solid var(--pc-border)', background: 'transparent', color: 'var(--pc-text-muted)', cursor: 'pointer', fontSize: 12.5 }}
          >
            취소
          </button>
          <button
            onClick={handleSave}
            style={{
              padding: '7px 18px', borderRadius: 8, border: 'none', cursor: 'pointer',
              background: saved ? 'var(--pc-spring)' : agentColor,
              color: '#1a1a22', fontWeight: 700, fontSize: 12.5,
              transition: 'background 0.2s',
            }}
          >
            {saved ? '✓ 저장됨' : '저장'}
          </button>
        </div>
      </div>

      <div style={{ maxWidth: 640 }}>

        {/* 기본 정보 */}
        <Section title="기본 정보">
          <Field label="이름">
            <input style={inputStyle} value={name} onChange={e => setName(e.target.value)} placeholder="에이전트 이름" />
          </Field>

          <Field label="설명">
            <textarea
              style={{ ...inputStyle, resize: 'vertical', minHeight: 72 }}
              value={description}
              onChange={e => setDescription(e.target.value)}
              placeholder="이 에이전트의 역할과 용도를 설명하세요"
            />
          </Field>

          <Field label="색상">
            <div style={{ display: 'flex', gap: 8 }}>
              {COLORS.map(c => (
                <button
                  key={c}
                  onClick={() => setColor(c)}
                  title={COLOR_LABELS[c]}
                  style={{
                    width: 32, height: 32, borderRadius: 8, border: color === c ? `2px solid ${COLOR_VARS[c]}` : '2px solid transparent',
                    background: COLOR_VARS[c], cursor: 'pointer', outline: 'none',
                    boxShadow: color === c ? `0 0 8px ${COLOR_VARS[c]}88` : 'none',
                    transition: 'all 0.15s',
                  }}
                />
              ))}
            </div>
          </Field>

          <Field label="프로바이더">
            <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              {PROVIDERS.map(p => (
                <button
                  key={p}
                  onClick={() => { setProvider(p); setModel(MODELS[p]?.[0] ?? ''); }}
                  style={{
                    padding: '7px 14px', borderRadius: 8, cursor: 'pointer', fontSize: 13,
                    border: provider === p ? `1px solid ${agentColor}` : '1px solid var(--pc-border)',
                    background: provider === p ? `${agentColor}20` : 'var(--pc-bg-elevated)',
                    color: provider === p ? agentColor : 'var(--pc-text-secondary)',
                    fontWeight: provider === p ? 600 : 400,
                    transition: 'all 0.15s',
                  }}
                >{p}</button>
              ))}
            </div>
          </Field>

          <Field label="모델">
            <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
              {(MODELS[provider] ?? []).map(m => (
                <button
                  key={m}
                  onClick={() => setModel(m)}
                  style={{
                    padding: '5px 11px', borderRadius: 20, cursor: 'pointer', fontSize: 12,
                    border: model === m ? `1px solid ${agentColor}` : '1px solid var(--pc-border)',
                    background: model === m ? `${agentColor}20` : 'var(--pc-bg-elevated)',
                    color: model === m ? agentColor : 'var(--pc-text-muted)',
                    fontWeight: model === m ? 600 : 400,
                    transition: 'all 0.15s',
                  }}
                >{m}</button>
              ))}
            </div>
          </Field>
        </Section>

        {/* 파일 접근 */}
        <Section title="파일 접근">
          <Field label="기본 작업 디렉토리" hint="에이전트가 파일 작업 시 기준으로 삼는 폴더 경로">
            <input
              style={inputStyle}
              value={policy.workingDir}
              onChange={e => policySet('workingDir', e.target.value)}
              placeholder="예: /Users/me/projects/myapp"
            />
          </Field>

          <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '10px 12px', borderRadius: 9, background: 'var(--pc-bg-elevated)', border: '1px solid var(--pc-border)' }}>
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--pc-text-primary)' }}>워크스페이스 내부만 허용</div>
              <div style={{ fontSize: 12, color: 'var(--pc-text-muted)', marginTop: 2 }}>켜면 작업 디렉토리 외부 절대 경로 접근을 차단합니다</div>
            </div>
            <button
              onClick={() => policySet('workspaceOnly', !policy.workspaceOnly)}
              style={{
                width: 44, height: 24, borderRadius: 12, border: 'none', cursor: 'pointer', padding: 0,
                background: policy.workspaceOnly ? agentColor : 'var(--pc-bg-input)',
                position: 'relative', flexShrink: 0, transition: 'background 0.2s',
              }}
            >
              <span style={{
                position: 'absolute', top: 3, left: policy.workspaceOnly ? 23 : 3,
                width: 18, height: 18, borderRadius: '50%', background: '#fff',
                transition: 'left 0.2s', boxShadow: '0 1px 3px rgba(0,0,0,0.3)',
              }} />
            </button>
          </div>

          <TagInput
            label="추가 접근 허용 폴더"
            hint="작업 디렉토리 외에 읽기/쓰기를 허용할 절대 경로 목록"
            tags={policy.allowedRoots}
            onChange={v => policySet('allowedRoots', v)}
            placeholder="/Users/me/shared-docs 추가 후 Enter"
          />
        </Section>

        {/* 명령어 권한 */}
        <Section title="명령어 권한">
          <TagInput
            label="허용 명령어"
            hint="빈 목록이면 모든 명령어 허용. 입력하면 해당 명령어만 실행 가능"
            tags={policy.allowedCommands}
            onChange={v => policySet('allowedCommands', v)}
            placeholder="git, npm, python 추가 후 Enter"
          />

          <Field label="쉘 타임아웃 (초)" hint="명령어 실행 최대 대기 시간">
            <input
              type="number" min={5} max={600}
              style={{ ...inputStyle, width: 120 }}
              value={policy.shellTimeoutSecs}
              onChange={e => policySet('shellTimeoutSecs', Number(e.target.value))}
            />
          </Field>
        </Section>

        {/* 도구 승인 */}
        <Section title="도구 승인">
          <TagInput
            label="자동 승인 도구"
            hint="확인 없이 바로 실행할 도구 이름 목록"
            tags={policy.autoApproveTools}
            onChange={v => policySet('autoApproveTools', v)}
            placeholder="file_read, web_search_tool 추가 후 Enter"
          />
          <TagInput
            label="항상 확인 도구"
            hint="'항상 허용'을 눌러도 매번 확인을 요구하는 도구 목록"
            tags={policy.alwaysAskTools}
            onChange={v => policySet('alwaysAskTools', v)}
            placeholder="shell_exec 추가 후 Enter"
          />
        </Section>

        {/* 시스템 프롬프트 */}
        <Section title="시스템 프롬프트">
          <Field label="사용자 정의 시스템 프롬프트" hint="비워두면 기본 시스템 프롬프트를 사용합니다">
            <textarea
              style={{ ...inputStyle, resize: 'vertical', minHeight: 140, fontFamily: 'var(--pc-font-mono)', fontSize: 12.5, lineHeight: 1.6 }}
              value={policy.systemPrompt}
              onChange={e => policySet('systemPrompt', e.target.value)}
              placeholder="당신은 코드 리뷰 전문가입니다. 한국어로 응답하고..."
            />
          </Field>
        </Section>

        {/* 위험 구역 */}
        <Section title="위험 구역">
          {!showDelete ? (
            <button
              onClick={() => setShowDelete(true)}
              style={{ padding: '8px 16px', borderRadius: 8, border: '1px solid rgba(195,64,67,0.4)', background: 'transparent', color: 'var(--color-status-error)', cursor: 'pointer', fontSize: 13, alignSelf: 'flex-start' }}
            >
              이 에이전트 삭제
            </button>
          ) : (
            <div style={{ padding: '14px 16px', borderRadius: 10, border: '1px solid rgba(195,64,67,0.35)', background: 'rgba(195,64,67,0.06)' }}>
              <div style={{ fontSize: 13, color: 'var(--pc-text-primary)', marginBottom: 12 }}>
                <b>{agent.name}</b>과 모든 대화 기록을 삭제합니다. 되돌릴 수 없습니다.
              </div>
              <div style={{ display: 'flex', gap: 8 }}>
                <button
                  onClick={handleDelete}
                  style={{ padding: '7px 16px', borderRadius: 8, border: 'none', background: 'var(--color-status-error)', color: '#fff', cursor: 'pointer', fontSize: 12.5, fontWeight: 700 }}
                >
                  삭제 확인
                </button>
                <button
                  onClick={() => setShowDelete(false)}
                  style={{ padding: '7px 14px', borderRadius: 8, border: '1px solid var(--pc-border)', background: 'transparent', color: 'var(--pc-text-muted)', cursor: 'pointer', fontSize: 12.5 }}
                >
                  취소
                </button>
              </div>
            </div>
          )}
        </Section>
      </div>
    </div>
  );
}
