const STRIPE_SECRET_KEY = Deno.env.get("STRIPE_SECRET_KEY")!;

const corsHeaders = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "authorization, x-client-info, apikey, content-type",
};

Deno.serve(async (req) => {
  if (req.method === "OPTIONS") {
    return new Response(null, { headers: corsHeaders });
  }

  try {
    const { priceId } = await req.json();
    
    // Force http://localhost:8000 for local dev
    let origin = req.headers.get("origin");
    if (!origin || !origin.startsWith("http")) {
      origin = "http://localhost:8000";
    }

    const authHeader = req.headers.get("Authorization");
    if (!authHeader) {
      return new Response(JSON.stringify({ error: "No auth header" }), {
        status: 401,
        headers: { ...corsHeaders, "Content-Type": "application/json" },
      });
    }
    
    const token = authHeader.replace("Bearer ", "");
    const payload = JSON.parse(atob(token.split(".")[1]));
    const email = payload.email;
    const userId = payload.sub;

    // Find or create customer
    const customersRes = await fetch(
      `https://api.stripe.com/v1/customers?email=${encodeURIComponent(email)}&limit=1`,
      { headers: { "Authorization": `Bearer ${STRIPE_SECRET_KEY}` } }
    );
    const customers = await customersRes.json();
    
    if (customers.error) {
      return new Response(JSON.stringify({ error: "Stripe customers error: " + customers.error.message }), {
        status: 400,
        headers: { ...corsHeaders, "Content-Type": "application/json" },
      });
    }

    let customerId: string;
    if (customers.data?.length > 0) {
      customerId = customers.data[0].id;
    } else {
      const createRes = await fetch("https://api.stripe.com/v1/customers", {
        method: "POST",
        headers: {
          "Authorization": `Bearer ${STRIPE_SECRET_KEY}`,
          "Content-Type": "application/x-www-form-urlencoded",
        },
        body: new URLSearchParams({ email, "metadata[supabase_user_id]": userId }),
      });
      const newCustomer = await createRes.json();
      if (newCustomer.error) {
        return new Response(JSON.stringify({ error: "Create customer error: " + newCustomer.error.message }), {
          status: 400,
          headers: { ...corsHeaders, "Content-Type": "application/json" },
        });
      }
      customerId = newCustomer.id;
    }

    // Create checkout session
    const sessionRes = await fetch("https://api.stripe.com/v1/checkout/sessions", {
      method: "POST",
      headers: {
        "Authorization": `Bearer ${STRIPE_SECRET_KEY}`,
        "Content-Type": "application/x-www-form-urlencoded",
      },
      body: new URLSearchParams({
        customer: customerId,
        "payment_method_types[0]": "card",
        "line_items[0][price]": priceId,
        "line_items[0][quantity]": "1",
        mode: "subscription",
        success_url: `https://www.usepkr.ai/success.html?session_id={CHECKOUT_SESSION_ID}`,
        cancel_url: `${origin}/`,
        "subscription_data[trial_period_days]": "7",
        "subscription_data[metadata][supabase_user_id]": userId,
      }),
    });
    const session = await sessionRes.json();
    
    if (session.error) {
      return new Response(JSON.stringify({ error: "Checkout error: " + session.error.message }), {
        status: 400,
        headers: { ...corsHeaders, "Content-Type": "application/json" },
      });
    }

    return new Response(JSON.stringify({ url: session.url }), {
      headers: { ...corsHeaders, "Content-Type": "application/json" },
    });
  } catch (error) {
    return new Response(JSON.stringify({ error: "Server error: " + error.message }), {
      status: 500,
      headers: { ...corsHeaders, "Content-Type": "application/json" },
    });
  }
});
