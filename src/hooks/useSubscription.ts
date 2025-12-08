// src/hooks/useSubscription.ts
import { useEffect, useState } from 'react';
import { supabase } from '../lib/supabase';
import { useAuthContext } from '../contexts/AuthContext';
import { PLANS, PlanId } from '../config/plans';

interface Subscription {
  plan: PlanId;
  status: string;
  trialEndsAt: string | null;
  currentPeriodEnd: string;
  cancelAtPeriodEnd: boolean;
}

interface Usage {
  handsAnalyzed: number;
  periodStart: string;
  periodEnd: string;
}

export function useSubscription() {
  const { user } = useAuthContext();
  const [subscription, setSubscription] = useState<Subscription | null>(null);
  const [usage, setUsage] = useState<Usage | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!user) {
      setSubscription(null);
      setUsage(null);
      setLoading(false);
      return;
    }

    async function fetchSubscription() {
      const { data: sub } = await supabase
        .from('subscriptions')
        .select('*')
        .eq('user_id', user.id)
        .single();

      if (sub) {
        setSubscription({
          plan: sub.plan as PlanId,
          status: sub.status,
          trialEndsAt: sub.trial_ends_at,
          currentPeriodEnd: sub.current_period_end,
          cancelAtPeriodEnd: sub.cancel_at_period_end,
        });
      }

      const { data: usageData } = await supabase
        .from('usage')
        .select('*')
        .eq('user_id', user.id)
        .single();

      if (usageData) {
        setUsage({
          handsAnalyzed: usageData.hands_analyzed,
          periodStart: usageData.period_start,
          periodEnd: usageData.period_end,
        });
      }

      setLoading(false);
    }

    fetchSubscription();
  }, [user]);

  const plan = subscription?.plan || 'basic';
  const limits = PLANS[plan].limits;
  const features = PLANS[plan].features;

  const canAnalyzeHand = () => {
    if (!usage) return true;
    return usage.handsAnalyzed < limits.handsPerMonth;
  };

  const hasFeature = (feature: keyof typeof features) => {
    return features[feature];
  };

  return {
    subscription,
    usage,
    loading,
    plan,
    limits,
    features,
    canAnalyzeHand,
    hasFeature,
  };
}