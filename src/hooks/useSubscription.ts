// src/hooks/useSubscription.ts
import { useEffect, useState, useCallback } from 'react';
import { supabase } from '../lib/supabase';
import { useAuthContext } from '../context/AuthContext';
import { PLANS, PlanId } from '../config/plan';

interface UserUsage {
  handsUsed: number;
  subscriptionTier: PlanId | null;
  periodStart: string;
}

// Tier limits (matches PLANS config)
const TIER_LIMITS: Record<PlanId | 'none', number> = {
  basic: 1000,
  pro: 2000,
  elite: 3000,
  none: 0,
};

// Helper: Get first day of current month as ISO string
function getFirstOfMonth(): string {
  const now = new Date();
  return new Date(now.getFullYear(), now.getMonth(), 1).toISOString();
}

// Helper: Check if date is from a previous month
function isPreviousMonth(dateString: string): boolean {
  const date = new Date(dateString);
  const now = new Date();
  return (
    date.getFullYear() < now.getFullYear() ||
    (date.getFullYear() === now.getFullYear() && date.getMonth() < now.getMonth())
  );
}

export function useSubscription() {
  const { user } = useAuthContext();
  const [userUsage, setUserUsage] = useState<UserUsage | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);


  // Fetch user usage with monthly reset logic
  const fetchUserUsage = useCallback(async () => {
    if (!user) {
      return null;
    }

    try {
      const { data: usageData, error: fetchError } = await supabase
        .from('user_usage')
        .select('*')
        .eq('user_id', user.id)
        .single();


      // Handle no rows found (new user)
      if (fetchError && fetchError.code === 'PGRST116') {
        // Create new usage record
        const newUsage = {
          user_id: user.id,
          hands_used: 0,
          subscription_tier: null,
          period_start: getFirstOfMonth(),
        };

        const { data: insertedUsage, error: insertError } = await supabase
          .from('user_usage')
          .insert(newUsage)
          .select()
          .single();

        if (insertError) {
          console.error('Error creating user_usage record:', insertError);
          return null;
        }

        return insertedUsage;
      }

      if (fetchError) {
        console.error('Error fetching user_usage:', fetchError);
        return null;
      }

      // Monthly reset: Check if period_start is from previous month
      if (usageData && isPreviousMonth(usageData.period_start)) {
        const { data: resetUsage, error: resetError } = await supabase
          .from('user_usage')
          .update({
            hands_used: 0,
            period_start: getFirstOfMonth(),
          })
          .eq('user_id', user.id)
          .select()
          .single();

        if (resetError) {
          console.error('Error resetting user_usage:', resetError);
          return usageData; // Return stale data rather than fail
        }

        return resetUsage;
      }

      return usageData;
    } catch (err) {
      console.error('Error in fetchUserUsage:', err);
      return null;
    }
  }, [user]);

  // Main fetch effect
  useEffect(() => {
    if (!user) {
      setUserUsage(null);
      setLoading(false);
      return;
    }

    async function fetchData() {
      setLoading(true);
      setError(null);

      try {
        const usageData = await fetchUserUsage();

        if (usageData) {
          const parsed = {
            handsUsed: usageData.hands_used,
            subscriptionTier: usageData.subscription_tier as PlanId | null,
            periodStart: usageData.period_start,
          };
          setUserUsage(parsed);
        } else {
          setUserUsage(null);
        }
      } catch (err) {
        setError('Failed to fetch user usage data');
        console.error(err);
      } finally {
        setLoading(false);
      }
    }

    fetchData();
  }, [user, fetchUserUsage]);

  // Derived values - null subscription = 0 hands allowed
  const plan = userUsage?.subscriptionTier || null;
  const tierLimit = plan ? TIER_LIMITS[plan] : TIER_LIMITS.none;
  const handsUsed = userUsage?.handsUsed ?? 0;
  const handsRemaining = Math.max(0, tierLimit - handsUsed);

  // Warning threshold: 100 hands remaining
  const isNearLimit = handsRemaining <= 100 && handsRemaining > 0;
  const isAtLimit = tierLimit > 0 ? handsRemaining === 0 : true; // No subscription = always at limit

  // Check if user can analyze a hand
  const canAnalyzeHand = useCallback((): boolean => {
    if (!plan) return false; // No subscription = no hands
    return handsUsed < tierLimit;
  }, [plan, handsUsed, tierLimit]);

  // Increment hands used (call after successful analysis)
  const incrementHandsUsed = useCallback(async (): Promise<boolean> => {
    if (!user) return false;

    const newCount = handsUsed + 1;

    const { error: updateError } = await supabase
      .from('user_usage')
      .update({ hands_used: newCount })
      .eq('user_id', user.id);

    if (updateError) {
      console.error('Error incrementing hands_used:', updateError);
      return false;
    }

    // Update local state
    setUserUsage((prev) =>
      prev ? { ...prev, handsUsed: newCount } : prev
    );

    return true;
  }, [user, handsUsed, tierLimit]);

  // Refresh usage (for re-checking after potential external changes)
  const refreshUsage = useCallback(async () => {
    const usageData = await fetchUserUsage();
    if (usageData) {
      setUserUsage({
        handsUsed: usageData.hands_used,
        subscriptionTier: usageData.subscription_tier as PlanId | null,
        periodStart: usageData.period_start,
      });
    }
  }, [fetchUserUsage]);

  // Feature and limits from plans config
  const limits = plan ? PLANS[plan].limits : { handsPerMonth: 0 };
  const features = plan ? PLANS[plan].features : null;

  const hasFeature = (feature: string): boolean => {
    if (!plan || !features) return false;
    return (features as Record<string, boolean>)[feature] ?? false;
  };

  return {
    // State
    usage: userUsage,
    loading,
    error,
    plan,
    limits,
    features,
    // Usage tracking
    tierLimit,
    handsUsed,
    handsRemaining,
    isNearLimit,
    isAtLimit,
    // Methods
    canAnalyzeHand,
    incrementHandsUsed,
    refreshUsage,
    hasFeature,
  };
}
