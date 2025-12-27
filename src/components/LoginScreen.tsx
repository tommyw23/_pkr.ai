import { useState } from "react";
import logo from "../images/pkr-logo.png";
import { supabase } from "../lib/supabase";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getCurrentWindow } from "@tauri-apps/api/window";

export default function LoginScreen() {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [successMessage, setSuccessMessage] = useState("");

  // Validation
  const validateEmail = (email: string): boolean => {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
    return emailRegex.test(email);
  };

  const validateForm = (): string | null => {
    if (!email.trim()) {
      return "Email is required";
    }
    if (!validateEmail(email)) {
      return "Please enter a valid email address";
    }
    if (!password) {
      return "Password is required";
    }
    if (password.length < 6) {
      return "Password must be at least 6 characters";
    }
    return null;
  };

  const handleSignIn = async () => {
    setError("");
    setSuccessMessage("");

    const validationError = validateForm();
    if (validationError) {
      setError(validationError);
      return;
    }

    setLoading(true);

    try {
      const { error: signInError } = await supabase.auth.signInWithPassword({
        email: email.trim(),
        password,
      });

      if (signInError) {
        console.error("Sign in error:", signInError);
        setError(signInError.message);
        return;
      }

      // AuthContext will detect the session change and update the UI
    } catch (err: any) {
      console.error("Sign in error:", err);
      setError(err.message || "Something went wrong");
    } finally {
      setLoading(false);
    }
  };

  const handleSignUp = async () => {
    setError("");
    setSuccessMessage("");

    const validationError = validateForm();
    if (validationError) {
      setError(validationError);
      return;
    }

    setLoading(true);

    try {
      const { data, error: signUpError } = await supabase.auth.signUp({
        email: email.trim(),
        password,
      });

      if (signUpError) {
        console.error("Sign up error:", signUpError);
        setError(signUpError.message);
        return;
      }

      // Check if email confirmation is required
      if (data.user && !data.session) {
        setSuccessMessage("Check your email for a confirmation link to complete sign up.");
        setEmail("");
        setPassword("");
      }
    } catch (err: any) {
      console.error("Sign up error:", err);
      setError(err.message || "Something went wrong");
    } finally {
      setLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !loading) {
      handleSignIn();
    }
  };

  const handleOpenTerms = async () => {
    try {
      await openUrl("https://usepkr.ai/terms");
    } catch (err) {
      console.error("Failed to open terms:", err);
    }
  };

  const handleOpenPrivacy = async () => {
    try {
      await openUrl("https://usepkr.ai/privacy");
    } catch (err) {
      console.error("Failed to open privacy:", err);
    }
  };

  const handleMinimize = async () => {
    try {
      await getCurrentWindow().minimize();
    } catch (err) {
      console.error("Failed to minimize:", err);
    }
  };

  const handleClose = async () => {
    try {
      await getCurrentWindow().close();
    } catch (err) {
      console.error("Failed to close:", err);
    }
  };

  return (
    <div style={styles.container}>
      {/* Draggable title bar with window controls */}
      <div
        data-tauri-drag-region
        style={styles.titleBar}
      >
        <div style={{ flex: 1 }} data-tauri-drag-region />
        <div style={styles.windowControls}>
          <button
            style={styles.windowButton}
            onClick={handleMinimize}
            title="Minimize"
            aria-label="Minimize"
          >
            −
          </button>
          <button
            style={styles.windowButton}
            onClick={handleClose}
            title="Close"
            aria-label="Close"
          >
            ✕
          </button>
        </div>
      </div>

      <div style={styles.content}>
        {/* Logo */}
        <img
          src={logo}
          alt="pkr.ai logo"
          style={styles.logo}
        />

        {/* Heading */}
        <h1 style={styles.heading}>Welcome to pkr.ai</h1>

        {/* Subheading */}
        <p style={styles.subheading}>The #1 undetectable AI for poker</p>

        {/* Success message */}
        {successMessage && <p style={styles.success}>{successMessage}</p>}

        {/* Error message */}
        {error && <p style={styles.error}>{error}</p>}

        {/* Email input */}
        <input
          type="email"
          placeholder="Email address"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={loading}
          style={styles.input}
        />

        {/* Password input */}
        <input
          type="password"
          placeholder="Password (min 6 characters)"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={loading}
          style={styles.input}
        />

        {/* Button container */}
        <div style={styles.buttonContainer}>
          {/* Sign in button */}
          <button
            style={styles.signInButton}
            onClick={handleSignIn}
            disabled={loading}
          >
            {loading ? "Loading..." : "Sign in"}
          </button>

          {/* Sign up button */}
          <button
            style={styles.signUpButton}
            onClick={handleSignUp}
            disabled={loading}
          >
            {loading ? "Loading..." : "Sign up"}
          </button>
        </div>

        {/* Terms and Privacy */}
        <p style={styles.terms}>
          By signing up, you agree to our{" "}
          <button style={styles.link} onClick={handleOpenTerms}>
            Terms of Service
          </button>{" "}
          and{" "}
          <button style={styles.link} onClick={handleOpenPrivacy}>
            Privacy Policy
          </button>
        </p>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    width: "100vw",
    height: "100vh",
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    background: "linear-gradient(180deg, #0C0F14 0%, #151921 100%)",
    fontFamily: "system-ui, -apple-system, sans-serif",
    pointerEvents: "auto",
  },
  titleBar: {
    display: "flex",
    alignItems: "center",
    justifyContent: "flex-end",
    padding: "8px 12px",
    cursor: "move",
    width: "100%",
  },
  windowControls: {
    display: "flex",
    gap: 8,
  },
  windowButton: {
    width: 26,
    height: 26,
    borderRadius: 6,
    border: "1px solid rgba(255, 255, 255, 0.1)",
    background: "rgba(12, 15, 20, 0.9)",
    color: "#E8EEF5",
    cursor: "pointer",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    fontSize: 13,
    lineHeight: 1,
  },
  content: {
    flex: 1,
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    textAlign: "center",
    padding: "40px",
    maxWidth: "400px",
    width: "100%",
  },
  logo: {
    width: 80,
    height: 80,
    borderRadius: 16,
    objectFit: "cover",
    filter: "drop-shadow(0 0 20px rgba(255, 0, 80, 0.5))",
    marginBottom: 24,
  },
  heading: {
    fontSize: 28,
    fontWeight: 700,
    color: "#E8EEF5",
    margin: "0 0 8px 0",
  },
  subheading: {
    fontSize: 16,
    fontWeight: 400,
    color: "#98A2B3",
    margin: "0 0 24px 0",
  },
  error: {
    fontSize: 14,
    color: "#EF4444",
    margin: "0 0 16px 0",
    padding: "8px 16px",
    background: "rgba(239, 68, 68, 0.1)",
    borderRadius: 8,
    width: "100%",
    maxWidth: 320,
  },
  success: {
    fontSize: 14,
    color: "#22C55E",
    margin: "0 0 16px 0",
    padding: "8px 16px",
    background: "rgba(34, 197, 94, 0.1)",
    borderRadius: 8,
    width: "100%",
    maxWidth: 320,
  },
  input: {
    width: "100%",
    maxWidth: 320,
    padding: "12px 16px",
    fontSize: 15,
    color: "#E8EEF5",
    background: "rgba(255, 255, 255, 0.05)",
    border: "1px solid rgba(255, 255, 255, 0.1)",
    borderRadius: 10,
    marginBottom: 12,
    outline: "none",
    transition: "border-color 0.2s ease",
  },
  buttonContainer: {
    display: "flex",
    gap: 12,
    marginTop: 8,
    marginBottom: 24,
    width: "100%",
    maxWidth: 320,
  },
  signInButton: {
    flex: 1,
    padding: "12px 20px",
    fontSize: 15,
    fontWeight: 600,
    color: "#FFFFFF",
    background: "linear-gradient(135deg, #2563EB 0%, #1D4ED8 100%)",
    border: "none",
    borderRadius: 10,
    cursor: "pointer",
    boxShadow: "0 4px 16px rgba(37, 99, 235, 0.3)",
    transition: "all 0.2s ease",
  },
  signUpButton: {
    flex: 1,
    padding: "12px 20px",
    fontSize: 15,
    fontWeight: 600,
    color: "#E8EEF5",
    background: "transparent",
    border: "1px solid rgba(255, 255, 255, 0.2)",
    borderRadius: 10,
    cursor: "pointer",
    transition: "all 0.2s ease",
  },
  terms: {
    fontSize: 12,
    color: "#6B7280",
    lineHeight: 1.5,
    maxWidth: 280,
  },
  link: {
    background: "none",
    border: "none",
    color: "#9CA3AF",
    cursor: "pointer",
    textDecoration: "underline",
    fontSize: 12,
    padding: 0,
  },
};
