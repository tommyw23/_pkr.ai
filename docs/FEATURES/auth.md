# Authentication

## Purpose
Allow users to create accounts and securely access pkr.ai features based on subscription status.

## Provider
**Supabase Auth**

## Flow
1. User lands on app → checks auth state
2. If not logged in → show login/signup modal
3. User submits email + password
4. Supabase validates credentials
5. Session stored in local storage
6. User redirected to main app (or subscription page if no active sub)

## Implementation
- **Client:** `@supabase/supabase-js`
- **Config:** `src/lib/supabase.ts`
- **Auth state:** React context or Zustand store

## Key Functions
```typescript
// Sign up
const { data, error } = await supabase.auth.signUp({ email, password })

// Sign in
const { data, error } = await supabase.auth.signInWithPassword({ email, password })

// Sign out
await supabase.auth.signOut()

// Get session
const { data: { session } } = await supabase.auth.getSession()

// Listen to auth changes
supabase.auth.onAuthStateChange((event, session) => { ... })
```

## Edge Cases
- **Password reset:** Use `supabase.auth.resetPasswordForEmail()`
- **Email already exists:** Handle error code and show message
- **Session expiry:** Auto-refresh handled by Supabase client
- **OAuth (future):** Google/Discord sign-in for easier onboarding

## Protected Routes
All routes except `/login` and `/` (landing) require authenticated session.

## Status
✅ Implemented and working
