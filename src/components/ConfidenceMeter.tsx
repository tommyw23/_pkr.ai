
export default function ConfidenceMeter({ value }: { value: number }) {
  const pct = Math.max(0, Math.min(1, value));
  return (
    <div style={{ width: 124 }}>
      <div style={{ fontSize: 12, fontWeight: 600, color: "var(--muted)" }}>STRENGTH</div>
      <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 4 }}>
        <div style={{ fontSize: 13, fontWeight: 700 }}>{Math.round(pct * 100)}%</div>
        <div style={{
          flex: 1, height: 8, borderRadius: 8, background: "#FFFFFF1A", overflow: "hidden"
        }}>
          <div style={{
            width: `${pct * 100}%`,
            height: "100%",
            background: "linear-gradient(90deg, #00FFA6 0%, #2DD4BF 50%, #60A5FA 100%)",
            transition: "width 120ms ease"
          }}/>
        </div>
      </div>
    </div>
  );
}
