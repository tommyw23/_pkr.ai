# Billing (Stripe)

## Purpose
Handle monthly subscriptions with three pricing tiers.

## Pricing Tiers
| Tier | Price | Features |
|------|-------|----------|
| Basic | $29/mo | Core features, limited analysis |
| Pro | $59/mo | Full analysis, priority support |
| Elite | $99/mo | All features, highest API limits |

## Provider
**Stripe** (Checkout + Customer Portal)

## Flow
1. User clicks "Subscribe" on pricing page
2. App creates Stripe Checkout session via backend
3. User redirected to Stripe-hosted checkout
4. On success → webhook fires → update Supabase user record
5. User redirected back to app with active subscription
6. Manage subscription via Stripe Customer Portal

## Implementation

### Checkout Session Creation
```typescript
// Backend endpoint
const session = await stripe.checkout.sessions.create({
  mode: 'subscription',
  customer_email: user.email,
  line_items: [{ price: priceId, quantity: 1 }],
  success_url: `${APP_URL}/success?session_id={CHECKOUT_SESSION_ID}`,
  cancel_url: `${APP_URL}/pricing`,
  metadata: { userId: user.id }
});
```

### Webhook Handling
Listen for:
- `checkout.session.completed` → activate subscription
- `customer.subscription.updated` → handle upgrades/downgrades
- `customer.subscription.deleted` → revoke access
- `invoice.payment_failed` → notify user, grace period

### Database Schema (Supabase)
```sql
-- users table extension
subscription_status: 'active' | 'canceled' | 'past_due' | null
subscription_tier: 'basic' | 'pro' | 'elite' | null
stripe_customer_id: string
subscription_end_date: timestamp
```

## Files Involved
- `src/lib/stripe.ts` - Client-side Stripe initialization
- `src/pages/Pricing.tsx` - Pricing UI
- Backend webhook handler (Tauri command or edge function)

## Edge Cases
- **Failed payment:** 3-day grace period, then downgrade to free
- **Upgrades:** Prorate immediately
- **Downgrades:** Take effect at end of billing period
- **Refunds:** Handle manually via Stripe dashboard

## Open Questions
- [ ] Free trial period? (7 days?)
- [ ] Annual pricing discount?

## Status
✅ Implemented - checkout flow working
⚠️ Need to verify webhook handling in production
