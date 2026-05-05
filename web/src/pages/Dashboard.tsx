import { useState, useEffect, useCallback } from 'react';
import type { StatusResponse, CostSummary, Session, ChannelDetail } from '@/types/api';
import { getStatus, getCost, getSessions, getChannels } from '@/lib/api';
import { useSSE } from '@/hooks/useSSE';
import { t } from '@/lib/i18n';

// ─── helpers ──────────────────────────────────────────────────────────────────

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (d > 0) return `${d}일 ${h}시간`;
  if (h > 0) return `${h}시간 ${m}분`;
  return `${m}분`;
}

function formatRelative(iso: string): string {
  try {
    const diff = Date.now() - new Date(iso).getTime();
    const s = Math.floor(diff / 1000);
    if (s < 60) return `${s}초 전`;
    const m = Math.floor(s / 60);
    if (m < 60) return `${m}분 전`;
    const h = Math.floor(m / 60);
    if (h < 24) return `${h}시간 전`;
    return `${Math.floor(h / 24)}일 전`;
  } catch { return iso; }
}

function healthStatus(s: string) {
  const lc = s.toLowerCase();
  if (lc === 'ok' || lc === 'healthy') return 'ok';
  if (lc === 'warn' || lc === 'warning' || lc === 'degraded') return 'warn';
  return 'err';
}

// ─── Sparkline ────────────────────────────────────────────────────────────────

function Sparkline({ data, color = 'var(--pc-accent)', h = 36 }: {
  data: number[];
  color?: string;
  h?: number;
}) {
  if (data.length < 2) return null;
  const max = Math.max(...data, 0.001);
  const W = 80;
  const pts = data.map((v, i) =>
    `${(i / (data.length - 1)) * W},${h - (v / max) * (h - 2) - 1}`
  );
  const ptsStr = pts.join(' ');
  const first = pts[0]!.split(',');
  const last = pts[pts.length - 1]!.split(',');
  const areaPath = `M${first[0]},${h} L${ptsStr.split(' ').join(' L')} L${last[0]},${h} Z`;
  const id = `sp${color.replace(/[^a-z0-9]/gi, '')}`;
  return (
    <svg width={W} height={h} viewBox={`0 0 ${W} ${h}`} style={{ overflow: 'visible', flexShrink: 0 }}>
      <defs>
        <linearGradient id={id} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity="0.22" />
          <stop offset="100%" stopColor={color} stopOpacity="0.01" />
        </linearGradient>
      </defs>
      <path d={areaPath} fill={`url(#${id})`} />
      <polyline points={ptsStr} fill="none" stroke={color} strokeWidth="1.5"
        strokeLinejoin="round" strokeLinecap="round" />
    </svg>
  );
}

// ─── Sessions Tab ─────────────────────────────────────────────────────────────

function SessionsTab() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<Session | null>(null);
  const { events } = useSSE({ filterTypes: ['session_update', 'session_created', 'session_closed'], autoConnect: true });
  const load = useCallback(() => {
    getSessions().then(d => { setSessions(d); setLoading(false); }).catch(e => { setError(e.message); setLoading(false); });
  }, []);
  useEffect(() => { load(); }, [load]);
  useEffect(() => { if (events.length > 0) load(); }, [events.length, load]);

  if (loading) return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: 120, color: 'var(--pc-text-muted)', fontSize: 13 }}>
      세션 불러오는 중…
    </div>
  );
  if (error) return (
    <div className="kw-card" style={{ padding: 14, color: 'var(--color-status-error)', borderColor: 'rgba(195,64,67,0.3)', background: 'rgba(195,64,67,0.05)' }}>
      세션 로드 실패: {error}
    </div>
  );

  return (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 260px', gap: 14 }}>
      <div className="kw-card" style={{ padding: 0, overflow: 'hidden' }}>
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 72px 100px', padding: '11px 18px', fontSize: 11, fontWeight: 600, color: 'var(--pc-text-muted)', textTransform: 'uppercase', letterSpacing: '0.07em', borderBottom: '1px solid var(--pc-separator)' }}>
          <div>세션 ID</div><div>메시지</div><div>마지막 활동</div>
        </div>
        {sessions.length === 0
          ? <div style={{ padding: '28px 18px', textAlign: 'center', color: 'var(--pc-text-faint)', fontSize: 13 }}>세션 없음</div>
          : sessions.map(s => (
            <div key={s.session_id} onClick={() => setSelected(s)} style={{
              display: 'grid', gridTemplateColumns: '1fr 72px 100px',
              padding: '13px 18px', fontSize: 13, cursor: 'pointer',
              borderBottom: '1px solid var(--pc-separator)',
              background: selected?.session_id === s.session_id ? 'rgba(126,156,216,0.08)' : 'transparent',
            }}
              onMouseEnter={e => { if (selected?.session_id !== s.session_id) (e.currentTarget as HTMLElement).style.background = 'var(--pc-hover)'; }}
              onMouseLeave={e => { if (selected?.session_id !== s.session_id) (e.currentTarget as HTMLElement).style.background = 'transparent'; }}
            >
              <div className="mono" style={{ fontSize: 12, color: 'var(--pc-text-secondary)' }}>{s.session_id.slice(0, 16)}…</div>
              <div style={{ fontWeight: 600 }}>{s.message_count}</div>
              <div className="muted">{formatRelative(s.last_activity)}</div>
            </div>
          ))}
      </div>
      <div className="kw-card" style={{ padding: 16 }}>
        <div className="card-h" style={{ marginBottom: 14 }}>세션 상세</div>
        {selected
          ? <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
            <div><div className="tiny" style={{ marginBottom: 3 }}>세션 ID</div><div className="mono" style={{ fontSize: 11, wordBreak: 'break-all', color: 'var(--pc-text-secondary)' }}>{selected.session_id}</div></div>
            <div><div className="tiny" style={{ marginBottom: 3 }}>메시지 수</div><div style={{ fontSize: 13, fontWeight: 600 }}>{selected.message_count}</div></div>
            <div><div className="tiny" style={{ marginBottom: 3 }}>마지막 활동</div><div style={{ fontSize: 13 }}>{formatRelative(selected.last_activity)}</div></div>
          </div>
          : <div style={{ color: 'var(--pc-text-faint)', fontSize: 13, textAlign: 'center', paddingTop: 24 }}>세션을 선택하세요</div>
        }
      </div>
    </div>
  );
}

// ─── Channels Tab ─────────────────────────────────────────────────────────────

function ChannelsTab() {
  const [channels, setChannels] = useState<ChannelDetail[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { events } = useSSE({ filterTypes: ['channel_update', 'channel_status'], autoConnect: true });
  const load = useCallback(() => {
    getChannels().then(d => { setChannels(d); setLoading(false); }).catch(e => { setError(e.message); setLoading(false); });
  }, []);
  useEffect(() => { load(); }, [load]);
  useEffect(() => { if (events.length > 0) load(); }, [events.length, load]);

  if (loading) return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: 120, color: 'var(--pc-text-muted)', fontSize: 13 }}>
      채널 불러오는 중…
    </div>
  );
  if (error) return (
    <div className="kw-card" style={{ padding: 14, color: 'var(--color-status-error)', borderColor: 'rgba(195,64,67,0.3)', background: 'rgba(195,64,67,0.05)' }}>
      채널 로드 실패: {error}
    </div>
  );
  if (channels.length === 0) return (
    <div className="kw-card" style={{ padding: 32, textAlign: 'center', color: 'var(--pc-text-faint)', fontSize: 13 }}>연결된 채널 없음</div>
  );

  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 12 }}>
      {channels.map(c => {
        const hs = healthStatus(c.health);
        return (
          <div key={c.name} className="kw-card fade-in" style={{ padding: 16 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 12 }}>
              <div style={{
                width: 34, height: 34, borderRadius: 9, fontWeight: 700, fontSize: 14,
                textTransform: 'uppercase' as const, display: 'flex', alignItems: 'center', justifyContent: 'center',
                background: hs === 'ok' ? 'rgba(152,187,108,0.12)' : hs === 'warn' ? 'rgba(230,195,132,0.12)' : 'rgba(195,64,67,0.12)',
                color: hs === 'ok' ? 'var(--color-status-success)' : hs === 'warn' ? 'var(--color-status-warning)' : 'var(--color-status-error)',
              }}>{c.name[0]}</div>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 14, fontWeight: 600, textTransform: 'capitalize' as const }}>{c.name}</div>
                <div className="tiny">{c.type}</div>
              </div>
              {hs === 'ok' && <span className="pill ok"><span className="d" />활성</span>}
              {hs === 'warn' && <span className="pill warn"><span className="d" />주의</span>}
              {hs === 'err' && <span className="pill err"><span className="d" />오류</span>}
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr', gap: 8, paddingTop: 10, borderTop: '1px solid var(--pc-separator)' }}>
              {[
                { k: '메시지', v: c.message_count.toLocaleString(), mono: true },
                { k: '마지막', v: c.last_message_at ? formatRelative(c.last_message_at) : '-' },
                { k: '상태', v: c.status, style: { color: hs === 'ok' ? 'var(--color-status-success)' : hs === 'warn' ? 'var(--color-status-warning)' : 'var(--color-status-error)', textTransform: 'capitalize' as const } },
              ].map(({ k, v, mono, style }) => (
                <div key={k}>
                  <div className="tiny" style={{ marginBottom: 2 }}>{k}</div>
                  <div className={mono ? 'mono' : ''} style={{ fontSize: 13, fontWeight: 600, ...style }}>{v}</div>
                </div>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ─── Overview ─────────────────────────────────────────────────────────────────

const SPARK_SHAPE = [0.4, 0.6, 0.9, 1.2, 1.0, 1.6, 1.4, 2.0, 2.2, 1.8, 2.6, 3.4, 3.1, 3.8];

function OverviewTab({ status, cost }: { status: StatusResponse; cost: CostSummary }) {
  const compEntries = Object.entries(status.health.components);
  const chanEntries = Object.entries(status.channels);
  const activeCount = chanEntries.filter(([, v]) => v).length;

  return (
    <>
      {/* Component status strip */}
      <div className="status-strip">
        {compEntries.length === 0
          ? <span className="tiny">컴포넌트 없음</span>
          : compEntries.map(([name, comp]) => {
            const hs = healthStatus(comp.status);
            const col = hs === 'ok' ? 'var(--color-status-success)' : hs === 'warn' ? 'var(--color-status-warning)' : 'var(--color-status-error)';
            return (
              <span key={name} style={{ display: 'inline-flex', alignItems: 'center', gap: 5, fontSize: 11.5, color: 'var(--pc-text-secondary)' }}>
                <span style={{ display: 'inline-block', width: 6, height: 6, borderRadius: '50%', background: col, boxShadow: `0 0 5px ${col}` }} />
                {name}
              </span>
            );
          })}
        <span style={{ marginLeft: 'auto', fontSize: 11, color: 'var(--pc-text-faint)' }}>
          가동 {formatUptime(status.uptime_seconds)}
        </span>
      </div>

      {/* 3-col stat cards */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 12 }}>
        {[
          { label: '세션 비용', value: `$${cost.session_cost_usd.toFixed(4)}`, sub: `${cost.total_tokens.toLocaleString()} 토큰`, color: 'var(--pc-accent)', spark: SPARK_SHAPE.map(v => v * cost.session_cost_usd + 0.001) },
          { label: '오늘 비용', value: `$${cost.daily_cost_usd.toFixed(3)}`, sub: `${cost.request_count}건 요청`, color: 'var(--pc-spring)', spark: SPARK_SHAPE.map(v => v * cost.daily_cost_usd + 0.001) },
          { label: '이번 달', value: `$${cost.monthly_cost_usd.toFixed(2)}`, sub: `${status.provider} · ${status.model}`, color: 'var(--pc-iris)', spark: SPARK_SHAPE },
        ].map(({ label, value, sub, color, spark }) => (
          <div key={label} className="kw-card">
            <div className="card-h" style={{ marginBottom: 8 }}>{label}</div>
            <div style={{ display: 'flex', alignItems: 'flex-end', justifyContent: 'space-between' }}>
              <div>
                <div className="big-num">{value}</div>
                <div className="tiny" style={{ marginTop: 3 }}>{sub}</div>
              </div>
              <Sparkline data={spark} color={color} />
            </div>
          </div>
        ))}
      </div>

      {/* Component health grid */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 10 }}>
        {compEntries.length === 0
          ? <div className="kw-card" style={{ gridColumn: '1/-1', padding: '28px 18px', textAlign: 'center', color: 'var(--pc-text-faint)', fontSize: 13 }}>컴포넌트 없음</div>
          : compEntries.map(([name, comp]) => {
            const hs = healthStatus(comp.status);
            const ok = hs === 'ok';
            const warn = hs === 'warn';
            return (
              <div key={name} className="kw-card fade-in" style={{
                padding: '14px 18px', display: 'flex', alignItems: 'center', gap: 14,
                borderColor: ok ? 'var(--pc-border)' : warn ? 'rgba(230,195,132,0.25)' : 'rgba(195,64,67,0.25)',
                background: ok ? 'var(--pc-bg-surface)' : warn ? 'rgba(230,195,132,0.05)' : 'rgba(195,64,67,0.05)',
              }}>
                <div style={{
                  width: 28, height: 28, borderRadius: 8, fontWeight: 700, fontSize: 14,
                  display: 'flex', alignItems: 'center', justifyContent: 'center',
                  background: ok ? 'rgba(152,187,108,0.15)' : warn ? 'rgba(230,195,132,0.15)' : 'rgba(195,64,67,0.15)',
                  color: ok ? 'var(--color-status-success)' : warn ? 'var(--color-status-warning)' : 'var(--color-status-error)',
                }}>{ok ? '✓' : warn ? '!' : '✕'}</div>
                <div style={{ flex: 1 }}>
                  <div style={{ fontSize: 13.5, fontWeight: 600, textTransform: 'capitalize' as const }}>{name}</div>
                  <div className="tiny" style={{ marginTop: 2 }}>
                    {comp.status}{comp.restart_count > 0 ? ` · 재시작 ${comp.restart_count}회` : ''}
                  </div>
                </div>
                {ok && <span className="pill ok"><span className="d" />OK</span>}
                {warn && <span className="pill warn"><span className="d" />주의</span>}
                {!ok && !warn && <span className="pill err"><span className="d" />오류</span>}
              </div>
            );
          })}
      </div>

      {/* Channels strip */}
      <div className="kw-card" style={{ padding: '14px 18px' }}>
        <div style={{ display: 'flex', alignItems: 'center', marginBottom: 12 }}>
          <div className="card-h" style={{ flex: 1 }}>채널</div>
          <span className="tiny">{activeCount}개 활성 / {chanEntries.length}개 전체</span>
        </div>
        <div style={{ display: 'flex', flexWrap: 'wrap' as const, gap: 8 }}>
          {chanEntries.length === 0
            ? <span className="tiny">채널 없음</span>
            : chanEntries.map(([name, active]) => (
              <span key={name} className={active ? 'pill ok' : 'pill'} style={active ? {} : { color: 'var(--pc-text-faint)', borderColor: 'var(--pc-border)' }}>
                <span className="d" style={active ? {} : { background: 'var(--pc-text-faint)' }} />
                {name}
              </span>
            ))}
        </div>
      </div>
    </>
  );
}

// ─── Main ─────────────────────────────────────────────────────────────────────

type TabId = 'overview' | 'sessions' | 'channels';
const TABS: { id: TabId; label: string }[] = [
  { id: 'overview', label: '개요' },
  { id: 'sessions', label: '세션' },
  { id: 'channels', label: '채널' },
];

export default function Dashboard() {
  const [status, setStatus] = useState<StatusResponse | null>(null);
  const [cost, setCost] = useState<CostSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<TabId>('overview');

  useEffect(() => {
    Promise.all([getStatus(), getCost()])
      .then(([s, c]) => { setStatus(s); setCost(c); })
      .catch(e => setError(e.message));
  }, []);

  if (error) return (
    <div style={{ padding: '24px 32px' }}>
      <div className="kw-card" style={{ padding: 16, color: 'var(--color-status-error)', borderColor: 'rgba(195,64,67,0.3)', background: 'rgba(195,64,67,0.05)' }}>
        {t('dashboard.load_error')}: {error}
      </div>
    </div>
  );

  if (!status || !cost) return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: 200 }}>
      <div style={{ fontSize: 13, color: 'var(--pc-text-muted)' }}>데이터 불러오는 중…</div>
    </div>
  );

  return (
    <div style={{ flex: 1, overflowY: 'auto' }}>
      <div className="page-head">
        <div style={{ flex: 1 }}>
          <div className="crumb">대시보드</div>
          <h1>안녕하세요</h1>
          <div className="sub">나래 에이전트 · {status.model ?? 'unknown'} · {status.provider ?? 'local'}</div>
        </div>
        <div style={{ display: 'flex', gap: 4, background: 'var(--pc-bg-elevated)', padding: 4, borderRadius: 10 }}>
          {TABS.map(tab => (
            <button key={tab.id} onClick={() => setActiveTab(tab.id)} style={{
              padding: '6px 14px', borderRadius: 7, border: 'none', cursor: 'pointer',
              fontSize: 12.5, fontWeight: 500, transition: 'all 0.15s',
              background: activeTab === tab.id ? 'var(--pc-bg-surface)' : 'transparent',
              color: activeTab === tab.id ? 'var(--pc-accent)' : 'var(--pc-text-muted)',
              boxShadow: activeTab === tab.id ? '0 1px 3px rgba(0,0,0,0.15)' : 'none',
            }}>
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      <div style={{ padding: '4px 32px 36px', display: 'flex', flexDirection: 'column', gap: 14 }}>
        {activeTab === 'overview' && <OverviewTab status={status} cost={cost} />}
        {activeTab === 'sessions' && <SessionsTab />}
        {activeTab === 'channels' && <ChannelsTab />}
      </div>
    </div>
  );
}
