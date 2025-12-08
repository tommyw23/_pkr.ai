// src/lib/checkout.ts
import { supabase } from './supabase';

export async function createCheckout(priceId: string) {
  const { data: { session } } = await supabase.auth.getSession();
  
  if (!session) {
    throw new Error('Must be logged in');
  }

  const response = await fetch(
    `${import.meta.env.VITE_SUPABASE_URL}/functions/v1/create-checkout`,
    {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${session.access_token}`,
      },
      body: JSON.stringify({ priceId }),
    }
  );

  const data = await response.json();
  
  if (data.error) {
    throw new Error(data.error);
  }

  window.location.href = data.url;
}
