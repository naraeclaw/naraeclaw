import { useState, useEffect, createContext, useContext } from 'react';
import { Outlet, useLocation } from 'react-router-dom';
import Sidebar from '@/components/layout/Sidebar';
import Header from '@/components/layout/Header';
import { ErrorBoundary } from '@/App';

export type AgentState = 'idle' | 'working' | 'error' | 'paused';

interface AgentStateCtx {
  agentState: AgentState;
  setAgentState: (s: AgentState) => void;
}

export const AgentStateContext = createContext<AgentStateCtx>({
  agentState: 'idle',
  setAgentState: () => {},
});

export const useAgentState = () => useContext(AgentStateContext);

const SIDEBAR_COLLAPSED_KEY = 'naraeclaw-sidebar-collapsed';

export default function Layout() {
  const { pathname } = useLocation();
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(() => {
    try {
      return localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === 'true';
    } catch {
      return false;
    }
  });
  const [agentState, setAgentState] = useState<AgentState>('idle');

  useEffect(() => {
    setSidebarOpen(false);
  }, [pathname]);

  useEffect(() => {
    try {
      localStorage.setItem(SIDEBAR_COLLAPSED_KEY, String(collapsed));
    } catch {
      // localStorage may not be available
    }
  }, [collapsed]);

  return (
    <AgentStateContext.Provider value={{ agentState, setAgentState }}>
      <div className="min-h-screen" style={{ background: 'var(--pc-bg-base)', color: 'var(--pc-text-primary)' }}>
        <Sidebar
          open={sidebarOpen}
          onClose={() => setSidebarOpen(false)}
          collapsed={collapsed}
          agentState={agentState}
        />

        <div
          className={`
            flex flex-col flex-1 min-w-0 h-screen transition-all duration-300 ease-in-out
            ${collapsed ? 'md:ml-14' : 'md:ml-[220px]'}
            ml-0
          `}
        >
          <Header
            onMenuToggle={() => setSidebarOpen((v) => !v)}
            onCollapseToggle={() => setCollapsed((c) => !c)}
            collapsed={collapsed}
          />

          <main className="flex-1 overflow-y-auto min-h-0">
            <ErrorBoundary key={pathname}>
              <Outlet />
            </ErrorBoundary>
          </main>
        </div>
      </div>
    </AgentStateContext.Provider>
  );
}
