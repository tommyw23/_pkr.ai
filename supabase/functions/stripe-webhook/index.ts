import { serve } from 'https://deno.land/std@0.168.0/http/server.ts';
import Stripe from 'https://esm.sh/stripe@14.14.0?target=deno';
import { createClient } from 'https://esm.sh/@supabase/supabase-js@2';

const stripe = new Stripe(Deno.env.get('STRIPE_SECRET_KEY')!, {
  apiVersion: '2023-10-16',
});

const supabaseUrl = Deno.env.get('SUPABASE_URL')!;
const supabaseServiceKey = Deno.env.get('SUPABASE_SERVICE_ROLE_KEY')!;
const webhookSecret = Deno.env.get('STRIPE_WEBHOOK_SECRET')!;

// Map Stripe price IDs to subscription tiers
const PRICE_TO_TIER: Record<string, string> = {
  // Basic $29/mo, $288/yr
  'price_1Sb3VuCJvJVawc7f8HAXUujg': 'basic',
  'price_1Sb3diCJvJVawc7fqkHnws2h': 'basic',
  // Pro $59/mo, $588/yr
  'price_1Sb3XRCJvJVawc7fxC71zJSI': 'pro',
  'price_1Sb3d0CJvJVawc7fOibJg6EB': 'pro',
  // Elite $99/mo, $948/yr
  'price_1Sb3Y2CJvJVawc7f8iXpM9b8': 'elite',
  'price_1Sb3cUCJvJVawc7fOZYvawAd': 'elite',
};

// Tier hierarchy for upgrade detection
const TIER_RANK: Record<string, number> = {
  'basic': 1,
  'pro': 2,
  'elite': 3,
};

function getTierFromPriceId(priceId: string): string | null {
  return PRICE_TO_TIER[priceId] || null;
}

function isUpgrade(oldTier: string | null, newTier: string): boolean {
  if (!oldTier) return true;
  return (TIER_RANK[newTier] || 0) > (TIER_RANK[oldTier] || 0);
}

async function getUserIdByEmail(
  supabase: ReturnType<typeof createClient>,
  email: string
): Promise<string | null> {
  const { data, error } = await supabase.auth.admin.listUsers();

  if (error) {
    console.error('Error listing users:', error.message);
    return null;
  }

  const user = data.users.find((u) => u.email === email);
  return user?.id || null;
}

async function getCustomerEmail(
  customerId: string
): Promise<string | null> {
  const customer = await stripe.customers.retrieve(customerId);
  if (customer.deleted) return null;
  return (customer as Stripe.Customer).email || null;
}

function getPeriodStart(): string {
  const firstOfMonth = new Date();
  firstOfMonth.setDate(1);
  firstOfMonth.setHours(0, 0, 0, 0);
  return firstOfMonth.toISOString();
}

serve(async (req) => {
  // Verify signature
  const signature = req.headers.get('stripe-signature');
  if (!signature) {
    return new Response(JSON.stringify({ error: 'Missing signature' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  const body = await req.text();
  let event: Stripe.Event;

  try {
    event = stripe.webhooks.constructEvent(body, signature, webhookSecret);
  } catch (err) {
    console.error('Webhook signature verification failed:', err.message);
    return new Response(JSON.stringify({ error: 'Invalid signature' }), {
      status: 400,
      headers: { 'Content-Type': 'application/json' },
    });
  }

  const supabase = createClient(supabaseUrl, supabaseServiceKey);

  try {
    switch (event.type) {
      case 'checkout.session.completed': {
        const session = event.data.object as Stripe.Checkout.Session;

        // Get customer email from session
        const customerEmail = session.customer_details?.email;
        if (!customerEmail) {
          console.error('No customer email in checkout session');
          break;
        }

        // Look up user by email
        const userId = await getUserIdByEmail(supabase, customerEmail);
        if (!userId) {
          console.error('User not found for email:', customerEmail);
          break;
        }

        // Get subscription to determine tier
        const subscriptionId = session.subscription as string;
        if (!subscriptionId) {
          console.error('No subscription ID in checkout session');
          break;
        }

        const subscription = await stripe.subscriptions.retrieve(subscriptionId);
        const priceId = subscription.items.data[0]?.price.id;
        const tier = getTierFromPriceId(priceId);

        if (!tier) {
          console.error('Unknown price ID:', priceId);
          break;
        }

        // Upsert user_usage: new subscription resets hands_used
        const { error: upsertError } = await supabase
          .from('user_usage')
          .upsert({
            user_id: userId,
            subscription_tier: tier,
            hands_used: 0,
            period_start: getPeriodStart(),
          }, { onConflict: 'user_id' });

        if (upsertError) {
          console.error('Error upserting user_usage:', upsertError.message);
        }
        break;
      }

      case 'customer.subscription.updated': {
        const subscription = event.data.object as Stripe.Subscription;

        // Get customer email
        const customerEmail = await getCustomerEmail(subscription.customer as string);
        if (!customerEmail) {
          console.error('Could not get customer email for subscription update');
          break;
        }

        // Look up user by email
        const userId = await getUserIdByEmail(supabase, customerEmail);
        if (!userId) {
          console.error('User not found for email:', customerEmail);
          break;
        }

        // Get new tier
        const priceId = subscription.items.data[0]?.price.id;
        const newTier = getTierFromPriceId(priceId);

        if (!newTier) {
          console.error('Unknown price ID:', priceId);
          break;
        }

        // Get current tier to check if upgrading
        const { data: currentUsage } = await supabase
          .from('user_usage')
          .select('subscription_tier')
          .eq('user_id', userId)
          .single();

        const oldTier = currentUsage?.subscription_tier || null;
        const shouldResetHands = isUpgrade(oldTier, newTier);

        // Update user_usage
        const updateData: Record<string, unknown> = {
          subscription_tier: newTier,
        };

        // Reset hands_used only on upgrade
        if (shouldResetHands) {
          updateData.hands_used = 0;
          updateData.period_start = getPeriodStart();
        }

        const { error: updateError } = await supabase
          .from('user_usage')
          .upsert({
            user_id: userId,
            ...updateData,
          }, { onConflict: 'user_id' });

        if (updateError) {
          console.error('Error updating user_usage:', updateError.message);
        }
        break;
      }

      case 'customer.subscription.deleted': {
        const subscription = event.data.object as Stripe.Subscription;

        // Get customer email
        const customerEmail = await getCustomerEmail(subscription.customer as string);
        if (!customerEmail) {
          console.error('Could not get customer email for subscription deletion');
          break;
        }

        // Look up user by email
        const userId = await getUserIdByEmail(supabase, customerEmail);
        if (!userId) {
          console.error('User not found for email:', customerEmail);
          break;
        }

        // Set subscription_tier to null (free tier = 0 hands allowed)
        const { error: updateError } = await supabase
          .from('user_usage')
          .update({
            subscription_tier: null,
          })
          .eq('user_id', userId);

        if (updateError) {
          console.error('Error clearing subscription tier:', updateError.message);
        }
        break;
      }

      default:
        // Unhandled event type - just acknowledge
        break;
    }

    return new Response(JSON.stringify({ received: true }), {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    });
  } catch (error) {
    console.error('Webhook processing error:', error.message);
    return new Response(JSON.stringify({ error: error.message }), {
      status: 500,
      headers: { 'Content-Type': 'application/json' },
    });
  }
});
