import { NavLink } from 'react-router-dom';

// ── Narae 날개 마크 ───────────────────────────────────────────────────
type AgentState = 'idle' | 'working' | 'error' | 'paused';

function NaraeMark({ size = 32, state = 'idle' as AgentState }: { size?: number; state?: AgentState }) {
  const accent = 'var(--pc-accent)';
  const ringClass = state === 'working' ? 'ring pulse'
                   : state === 'error'   ? 'ring error'
                   : 'ring';
  return (
    <span className="sb-mark">
      <span className={ringClass} style={{
        position: 'absolute',
        inset: -Math.round(size * 0.15),
        borderRadius: '50%',
        background: state === 'error'
          ? 'radial-gradient(circle, rgba(195,64,67,0.30) 0%, transparent 70%)'
          : 'radial-gradient(circle, var(--pc-accent-glow) 0%, transparent 70%)',
        animation: state === 'idle' ? 'none'
                  : state === 'error' ? 'narae-pulse 0.9s ease-in-out infinite'
                  : 'narae-pulse 2.4s ease-in-out infinite',
        opacity: state === 'idle' ? 0.5 : 1,
      }} />
      <svg width={size} height={size} viewBox="0 0 32 32" style={{ position: 'relative', zIndex: 1 }}>
        <defs>
          <linearGradient id="naraeGrad" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stopColor={accent} stopOpacity="1"/>
            <stop offset="100%" stopColor="var(--pc-iris)" stopOpacity="0.85"/>
          </linearGradient>
        </defs>
        <path d="M5 22 Q5 10, 16 6 Q22 8, 24 14 L20 14 Q18 11, 14 12 Q9 14, 9 22 Z"
              fill="url(#naraeGrad)" opacity="0.95"/>
        <path d="M11 22 Q11 16, 17 14 Q21 14, 22 18 L19 18 Q18 16.5, 16 17 Q14 18, 14 22 Z"
              fill={accent} opacity="0.9"/>
        <circle cx="24.5" cy="14" r="1.5" fill={accent}/>
      </svg>
    </span>
  );
}

// ── 아이콘 ────────────────────────────────────────────────────────────
function Icon({ d, size = 16 }: { d: React.ReactNode; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none"
         stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      {d}
    </svg>
  );
}

const Icons = {
  profiles:  <Icon d={<><circle cx="9" cy="7" r="4"/><path d="M3 21v-2a4 4 0 014-4h4a4 4 0 014 4v2"/><path d="M16 3.13a4 4 0 010 7.75"/><path d="M21 21v-2a4 4 0 00-3-3.87"/></>}/>,
  dashboard: <Icon d={<><rect x="3" y="3" width="7" height="9" rx="1.5"/><rect x="14" y="3" width="7" height="5" rx="1.5"/><rect x="14" y="12" width="7" height="9" rx="1.5"/><rect x="3" y="16" width="7" height="5" rx="1.5"/></>}/>,
  chat:      <Icon d={<><path d="M21 12c0 4.4-4 8-9 8-1.3 0-2.5-.2-3.6-.6L4 21l1.2-3.8C3.8 15.8 3 14 3 12c0-4.4 4-8 9-8s9 3.6 9 8z"/></>}/>,
  memory:    <Icon d={<><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></>}/>,
  cron:      <Icon d={<><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></>}/>,
  channels:  <Icon d={<><path d="M2 12h4M18 12h4"/><path d="M5 5l3 3M16 16l3 3M5 19l3-3M16 8l3-3"/><circle cx="12" cy="12" r="3"/></>}/>,
  remote:    <Icon d={<><rect x="3" y="6" width="18" height="12" rx="2"/><circle cx="8" cy="12" r="1.4" fill="currentColor"/><path d="M12 12h6"/></>}/>,
  cost:      <Icon d={<><circle cx="12" cy="12" r="9"/><path d="M9 9h4.5a2 2 0 010 4H9m0 0h4.5a2 2 0 010 4H9m3-10v12"/></>}/>,
  logs:      <Icon d={<><path d="M5 4h11l3 3v13a1 1 0 01-1 1H5a1 1 0 01-1-1V5a1 1 0 011-1z"/><path d="M8 10h8M8 14h8M8 18h5"/></>}/>,
  config:    <Icon d={<><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 00.3 1.8l.1.1a2 2 0 01-2.8 2.8l-.1-.1a1.7 1.7 0 00-1.8-.3 1.7 1.7 0 00-1 1.5V21a2 2 0 11-4 0v-.1a1.7 1.7 0 00-1-1.5 1.7 1.7 0 00-1.8.3l-.1.1a2 2 0 01-2.8-2.8l.1-.1a1.7 1.7 0 00.3-1.8 1.7 1.7 0 00-1.5-1H3a2 2 0 110-4h.1a1.7 1.7 0 001.5-1 1.7 1.7 0 00-.3-1.8l-.1-.1a2 2 0 012.8-2.8l.1.1a1.7 1.7 0 001.8.3h0a1.7 1.7 0 001-1.5V3a2 2 0 114 0v.1a1.7 1.7 0 001 1.5h0a1.7 1.7 0 001.8-.3l.1-.1a2 2 0 012.8 2.8l-.1.1a1.7 1.7 0 00-.3 1.8v0a1.7 1.7 0 001.5 1H21a2 2 0 110 4h-.1a1.7 1.7 0 00-1.5 1z"/></>}/>,
  doctor:    <Icon d={<><path d="M12 2l9 4v6c0 5-3.5 9-9 10-5.5-1-9-5-9-10V6l9-4z"/><path d="M9 12h6M12 9v6"/></>}/>,
  wiki:      <Icon d={<><path d="M4 4.5A2.5 2.5 0 016.5 2H20v18H6.5A2.5 2.5 0 014 22.5v-18z"/><path d="M4 19.5A2.5 2.5 0 016.5 17H20"/></>}/>,
  terminal:  <Icon d={<><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></>}/>,
  puzzle:    <Icon d={<><path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/></>}/>,
};

// ── Nav 정의 ──────────────────────────────────────────────────────────
const workItems = [
  { to: '/',         label: '대시보드', icon: Icons.dashboard },
  { to: '/agent',    label: '에이전트', icon: Icons.chat },
  { to: '/profiles', label: '프로필',   icon: Icons.profiles },
  { to: '/cron',     label: '스케줄',   icon: Icons.cron },
];

const sysItems = [
  { to: '/channels',     label: '채널',       icon: Icons.channels },
  { to: '/remote',       label: '원격 서버',   icon: Icons.remote },
  { to: '/memory',       label: '메모리',      icon: Icons.memory },
  { to: '/cli-tools',    label: 'CLI 도구',   icon: Icons.terminal },
  { to: '/cost',         label: '비용',        icon: Icons.cost },
  { to: '/logs',         label: '로그',        icon: Icons.logs },
  { to: '/doctor',       label: '진단',        icon: Icons.doctor },
  { to: '/integrations', label: '연동',        icon: Icons.puzzle },
  { to: '/config',       label: '설정',        icon: Icons.config },
];

interface SidebarProps {
  open: boolean;
  onClose: () => void;
  collapsed: boolean;
  agentState?: AgentState;
}

export default function Sidebar({ open, onClose, collapsed, agentState = 'idle' }: SidebarProps) {
  const stateLabel: Record<AgentState, string> = {
    idle: '대기 중',
    working: '작업 중',
    error: '오류',
    paused: '일시 정지',
  };
  const stateColor = agentState === 'error' ? 'var(--color-status-error)'
                   : agentState === 'working' ? 'var(--pc-accent)'
                   : 'var(--color-status-success)';

  function SidebarInner({ c }: { c: boolean }) {
    return (
      <div className="sidebar" style={{
        width: c ? 56 : 220,
        padding: c ? '18px 8px 14px' : '18px 12px 14px',
      }}>
        {/* Brand */}
        <div className="sb-brand" style={{ gap: c ? 0 : 10, paddingLeft: c ? 4 : 8, justifyContent: c ? 'center' : undefined }}>
          <NaraeMark size={c ? 28 : 36} state={agentState} />
          {!c && (
            <div>
              <div className="sb-name">나래클로</div>
              <div className="sb-status">
                <span style={{
                  display: 'inline-block', width: 5, height: 5, borderRadius: '50%',
                  background: stateColor,
                  boxShadow: `0 0 6px ${stateColor}`,
                  marginRight: 6, verticalAlign: 1,
                }}/>
                {stateLabel[agentState]}
              </div>
            </div>
          )}
        </div>

        {!c && <div className="sb-section">작업</div>}
        {workItems.map((item) => (
          <NavItem key={item.to} item={item} collapsed={c} onClick={onClose} />
        ))}

        {!c && <div className="sb-section">시스템</div>}
        {sysItems.map((item) => (
          <NavItem key={item.to} item={item} collapsed={c} onClick={onClose} />
        ))}

        <div style={{ flex: 1 }} />

        {!c && (
          <div style={{
            display: 'flex', alignItems: 'center', gap: 10, padding: '10px 8px',
            borderTop: '1px solid var(--pc-separator)', marginTop: 8,
          }}>
            <div style={{
              width: 28, height: 28, borderRadius: 8,
              background: 'linear-gradient(135deg, var(--pc-iris), var(--pc-sakura))',
              display: 'flex', alignItems: 'center', justifyContent: 'center',
              fontWeight: 700, fontSize: 11, color: '#1a1a22', flexShrink: 0,
            }}>JH</div>
            <div style={{ minWidth: 0, flex: 1 }}>
              <div style={{ fontSize: 12, fontWeight: 600, color: 'var(--pc-text-primary)' }}>지훈</div>
              <div style={{ fontSize: 10.5, color: 'var(--pc-text-muted)' }}>로컬 호스트</div>
            </div>
          </div>
        )}
      </div>
    );
  }

  return (
    <>
      {open && (
        <div
          className="md:hidden fixed inset-0 z-40 bg-black/60 backdrop-blur-sm"
          onClick={onClose}
          onKeyDown={(e) => { if (e.key === 'Escape') onClose(); }}
          role="button"
          tabIndex={-1}
          aria-label="Close menu"
        />
      )}

      <aside
        className="hidden md:flex fixed top-0 left-0 h-screen z-50 transition-all duration-300 ease-in-out"
        style={{ width: collapsed ? 56 : 220 }}
        aria-label={collapsed ? 'Collapsed sidebar' : 'Main sidebar'}
      >
        <SidebarInner c={collapsed} />
      </aside>

      <aside
        className={[
          'md:hidden fixed top-0 left-0 h-screen z-50 transition-transform duration-200 ease-out',
          open ? 'translate-x-0' : '-translate-x-full',
        ].join(' ')}
        style={{ width: 220 }}
        aria-label="Mobile menu"
      >
        <SidebarInner c={false} />
      </aside>
    </>
  );
}

function NavItem({
  item,
  collapsed,
  onClick,
}: {
  item: { to: string; label: string; icon: React.ReactNode };
  collapsed: boolean;
  onClick: () => void;
}) {
  return (
    <NavLink
      to={item.to}
      end={item.to === '/'}
      onClick={onClick}
      className={({ isActive }) =>
        ['sb-item', isActive ? 'active' : ''].filter(Boolean).join(' ')
      }
      style={{ justifyContent: collapsed ? 'center' : undefined }}
      title={collapsed ? item.label : undefined}
    >
      {item.icon}
      {!collapsed && <span>{item.label}</span>}
    </NavLink>
  );
}
