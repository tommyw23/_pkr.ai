import { openUrl } from '@tauri-apps/plugin-opener';

interface LimitReachedToastProps {
  tierLimit: number;
  currentPlan: string | null;
  onDismiss: () => void;
}

export default function LimitReachedToast({
  tierLimit,
  currentPlan,
  onDismiss,
}: LimitReachedToastProps) {
  const handleUpgrade = async () => {
    try {
      await openUrl('https://usepkr.ai/#pricing');
    } catch (err) {
      console.error('Failed to open pricing page:', err);
    }
  };

  const planName = currentPlan
    ? currentPlan.charAt(0).toUpperCase() + currentPlan.slice(1)
    : 'Free';

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
          border: '1px solid rgba(239, 68, 68, 0.4)',
        }}
      >
        <div
          style={{
            width: 36,
            height: 36,
            borderRadius: 8,
            background: '#EF4444',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: 16,
            fontWeight: 700,
            color: '#FFFFFF',
            flexShrink: 0,
          }}
        >
          0
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
            Monthly Limit Reached
          </div>
          <div style={{ fontSize: 12, color: '#98A2B3' }}>
            You've used all{' '}
            <strong style={{ color: '#EF4444' }}>{tierLimit}</strong> hands in
            your {planName} plan.{' '}
            <span
              style={{
                color: '#3B82F6',
                cursor: 'pointer',
                textDecoration: 'underline',
              }}
              onClick={handleUpgrade}
            >
              Upgrade for more
            </span>
          </div>
        </div>
        <button
          onClick={onDismiss}
          style={{
            padding: '6px 12px',
            background: '#EF4444',
            border: 'none',
            borderRadius: 6,
            color: '#FFFFFF',
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
