import { useState, useEffect } from 'react';
import { isTauri } from '../lib/tauri';

interface ChannelInfo {
  id: string;
  name: string;
  enabled: boolean;
  connected: boolean;
  needs: string[];
  description: string;
}

const DEFAULT_CHANNELS: ChannelInfo[] = [
  {
    id: 'webhook',
    name: 'Webhook',
    enabled: false,
    connected: false,
    needs: ['secret'],
    description: 'HTTP Webhook으로 에이전트에 메시지를 보냅니다. 비밀 키(선택)로 요청을 검증합니다.',
  },
  {
    id: 'mqtt',
    name: 'MQTT',
    enabled: false,
    connected: false,
    needs: ['broker_url', 'topics'],
    description: 'MQTT 브로커를 통해 IoT/서버 환경에서 에이전트와 통신합니다.',
  },
];

export default function ChannelSetup() {
  const [channels, setChannels] = useState<ChannelInfo[]>(DEFAULT_CHANNELS);
  const [editing, setEditing] = useState<string | null>(null);
  // Webhook fields
  const [webhookPort, setWebhookPort] = useState('42618');
  const [secret, setSecret] = useState('');
  // MQTT fields
  const [brokerUrl, setBrokerUrl] = useState('mqtt://');
  const [mqttTopics, setMqttTopics] = useState('naraeclaw/#');
  const [mqttUser, setMqttUser] = useState('');
  const [mqttPass, setMqttPass] = useState('');
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState('');

  useEffect(() => {
    if (!isTauri()) return;
    (async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const list = await invoke<ChannelInfo[]>('get_channels');
        if (list.length > 0) {
          setChannels(DEFAULT_CHANNELS.map(d => {
            const live = list.find(l => l.id === d.id);
            return live ? { ...d, enabled: live.enabled, connected: live.connected } : d;
          }));
        }
      } catch {}
    })();
  }, []);

  const clearForm = () => {
    setWebhookPort('42618');
    setSecret('');
    setBrokerUrl('mqtt://');
    setMqttTopics('naraeclaw/#');
    setMqttUser('');
    setMqttPass('');
    setMsg('');
  };

  const save = async (channelId: string) => {
    setSaving(true);
    setMsg('');
    try {
      if (isTauri()) {
        const { invoke } = await import('@tauri-apps/api/core');

        let payload: Record<string, unknown>;
        if (channelId === 'webhook') {
          const port = parseInt(webhookPort, 10);
          if (isNaN(port) || port < 1 || port > 65535) {
            setMsg('포트 번호는 1–65535 사이여야 합니다');
            setSaving(false);
            return;
          }
          payload = { channel: channelId, port, secret: secret || null };
        } else {
          const topicsList = mqttTopics
            .split(',')
            .map(t => t.trim())
            .filter(Boolean);
          if (topicsList.length === 0) {
            setMsg('최소 하나의 topic을 입력해야 합니다');
            setSaving(false);
            return;
          }
          payload = {
            channel: channelId,
            broker_url: brokerUrl || null,
            topics: topicsList,
            username: mqttUser || null,
            password: mqttPass || null,
          };
        }

        const result = await invoke<string>('save_channel', { settings: payload });
        setMsg(result + ' — 연결 확인 중...');
        await new Promise(r => setTimeout(r, 3000));
        try {
          const list = await invoke<ChannelInfo[]>('get_channels');
          if (list.length > 0) {
            setChannels(DEFAULT_CHANNELS.map(d => {
              const live = list.find(l => l.id === d.id);
              return live ? { ...d, enabled: live.enabled, connected: live.connected } : d;
            }));
            const ch = list.find(c => c.id === channelId);
            setMsg(ch?.connected ? `${channelId} 연결 성공! ✅` : `${channelId} 저장됨 (연결 확인 중...)`);
          }
        } catch {}
      } else {
        setMsg('Tauri 환경에서만 저장 가능합니다');
      }
      setEditing(null);
      clearForm();
    } catch (e: unknown) {
      setMsg(`오류: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setSaving(false);
    }
  };

  const isSaveDisabled = (channelId: string): boolean => {
    if (saving) return true;
    if (channelId === 'mqtt') return !brokerUrl || !mqttTopics.trim();
    return false;
  };

  return (
    <div style={{ padding: 24, maxWidth: 600 }}>
      <h2 style={{ fontSize: 20, marginBottom: 4 }}>채널 연동</h2>
      <p style={{ color: '#888', fontSize: 14, marginBottom: 20 }}>채널을 연결하면 외부에서 에이전트에 메시지를 보내거나 응답을 받을 수 있습니다</p>
      {channels.map(ch => (
        <div key={ch.id} style={cardStyle}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <div>
              <strong>{ch.name}</strong>
              <span style={{ marginLeft: 8, fontSize: 12, color: ch.connected ? '#4a9eff' : ch.enabled ? '#f0ad4e' : '#666' }}>
                {ch.connected ? '● 연결됨' : ch.enabled ? '● 설정됨' : '○ 미설정'}
              </span>
            </div>
            <button onClick={() => { setEditing(editing === ch.id ? null : ch.id); clearForm(); }} style={btnSmall}>
              {editing === ch.id ? '닫기' : ch.enabled ? '수정' : '연결하기'}
            </button>
          </div>
          {editing === ch.id && (
            <div style={{ marginTop: 12 }}>
              <p style={{ fontSize: 13, color: '#888', marginBottom: 8 }}>{ch.description}</p>

              {ch.id === 'webhook' && (
                <>
                  <label style={labelStyle}>포트 (기본: 42618)</label>
                  <input
                    placeholder="42618"
                    type="number"
                    value={webhookPort}
                    onChange={e => setWebhookPort(e.target.value)}
                    style={{ ...inputStyle, width: 120 }}
                    min={1}
                    max={65535}
                  />
                  <label style={{ ...labelStyle, marginTop: 8 }}>비밀 키 (선택)</label>
                  <input
                    placeholder="HMAC 서명 검증용 시크릿"
                    type="password"
                    value={secret}
                    onChange={e => setSecret(e.target.value)}
                    style={inputStyle}
                  />
                </>
              )}

              {ch.id === 'mqtt' && (
                <>
                  <label style={labelStyle}>Broker URL <span style={{ color: '#e94560' }}>*</span></label>
                  <input
                    placeholder="mqtt://localhost:1883"
                    value={brokerUrl}
                    onChange={e => setBrokerUrl(e.target.value)}
                    style={inputStyle}
                  />
                  <label style={{ ...labelStyle, marginTop: 8 }}>Topics <span style={{ color: '#e94560' }}>*</span> (쉼표로 구분)</label>
                  <input
                    placeholder="naraeclaw/#, alerts/+"
                    value={mqttTopics}
                    onChange={e => setMqttTopics(e.target.value)}
                    style={inputStyle}
                  />
                  <label style={{ ...labelStyle, marginTop: 8 }}>사용자명 (선택)</label>
                  <input
                    placeholder="사용자명"
                    value={mqttUser}
                    onChange={e => setMqttUser(e.target.value)}
                    style={inputStyle}
                  />
                  <label style={{ ...labelStyle, marginTop: 8 }}>비밀번호 (선택)</label>
                  <input
                    placeholder="비밀번호"
                    type="password"
                    value={mqttPass}
                    onChange={e => setMqttPass(e.target.value)}
                    style={inputStyle}
                  />
                </>
              )}

              <div style={{ marginTop: 10 }}>
                <button
                  onClick={() => save(ch.id)}
                  disabled={isSaveDisabled(ch.id)}
                  style={{ ...btnPrimary, opacity: isSaveDisabled(ch.id) ? 0.5 : 1 }}
                >
                  {saving ? '저장 중...' : '저장'}
                </button>
              </div>
            </div>
          )}
        </div>
      ))}
      {msg && <p style={{ marginTop: 12, fontSize: 14, color: msg.startsWith('오류') ? '#e94560' : '#4a9eff' }}>{msg}</p>}
    </div>
  );
}

const cardStyle: React.CSSProperties = { background: '#1a1a2e', padding: 16, borderRadius: 8, marginBottom: 8 };
const inputStyle: React.CSSProperties = { width: '100%', padding: '8px 12px', background: '#0f0f1a', border: '1px solid #2a2a3e', borderRadius: 6, color: '#e0e0e0', fontSize: 14, boxSizing: 'border-box' as const };
const labelStyle: React.CSSProperties = { display: 'block', fontSize: 12, color: '#888', marginBottom: 4 };
const btnPrimary: React.CSSProperties = { background: '#4a9eff', color: '#fff', border: 'none', borderRadius: 6, padding: '8px 16px', cursor: 'pointer' };
const btnSmall: React.CSSProperties = { background: '#2a2a3e', color: '#ccc', border: '1px solid #3a3a4e', borderRadius: 6, padding: '6px 14px', fontSize: 13, cursor: 'pointer' };
