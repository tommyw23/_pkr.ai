import { openUrl } from '@tauri-apps/plugin-opener';

interface LowHandsWarningProps {
  handsRemaining: number;
  onDismiss: () => void;
}

export default function LowHandsWarning({
  handsRemaining,
  onDismiss,
}: LowHandsWarningProps) {
  const handleUpgrade = async () => {
    try {
      await openUrl('https://usepkr.ai/#pricing');
    } catch (err) {
      console.error('Failed to open pricing page:', err);
    }
  };

  return (
    <div
      style={{
        marginTop: 12,
        pointerEvents: 'auto',
        animation: 'slideDown 0.2s ease-out',
      }}
    >
      <div
        className="pkr-frost-strong"
        style={{
          width: 560,
          padding: '12px 16px',
          borderRadius: 12,
          display: 'flex',
          alignItems: 'center',
          gap: 12,
          border: '1px solid rgba(251, 191, 36, 0.4)',
        }}
      >
        <div
          style={{
            width: 36,
            height: 36,
            borderRadius: 8,
            background: '#F59E0B',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: 18,
            flexShrink: 0,
          }}
        >
          <span role="img" aria-label="warning">
            !!
          </span>
        </div>
        <div style={{ flex: 1 }}>
          <div
            style={{
              fontSize: 14,
              fontWeight: 600,
              color: '#E8EEF5',
              marginBottom: 2,
            }}
          >
            Running Low on Hands
          </div>
          <div style={{ fontSize: 12, color: '#98A2B3' }}>
            You have{' '}
            <strong style={{ color: '#F59E0B' }}>{handsRemaining}</strong> hands
            remaining this month.{' '}
            <span
              style={{
                color: '#3B82F6',
                cursor: 'pointer',
                textDecoration: 'underline',
              }}
              onClick={handleUpgrade}
            >
              Upgrade now
            </span>
          </div>
        </div>
        <button
          onClick={onDismiss}
          style={{
            padding: '6px 12px',
            background: '#F59E0B',
            border: 'none',
            borderRadius: 6,
            color: '#000000',
            cursor: 'pointer',
            fontSize: 12,
            fontWeight: 600,
            flexShrink: 0,
          }}
        >
          Got it
        </button>
      </div>
    </div>
  );
}
