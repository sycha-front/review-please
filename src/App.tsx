import { useReviewDump } from "./hooks/useReviewDump";

function App() {
  const { error, isLoading, snapshot } = useReviewDump();

  return (
    <main
      style={{
        minHeight: "100vh",
        margin: 0,
        background: "#ffffff",
        padding: "24px",
        boxSizing: "border-box",
      }}
    >
      <div
        style={{
          width: "100%",
          minHeight: "calc(100vh - 48px)",
          border: "1px solid #e5e7eb",
          borderRadius: "12px",
          background: "#ffffff",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "#111827",
          fontSize: "24px",
          fontWeight: 600,
        }}
        >
        Main
      </div>
      <div
        style={{
          marginTop: "12px",
          color: "#4b5563",
          fontSize: "12px",
        }}
      >
        {isLoading && "Loading review data..."}
        {!isLoading && error && `Error: ${error}`}
        {!isLoading && !error && snapshot && `Pending ${snapshot.pending.length} / Done ${snapshot.done.length}`}
      </div>
    </main>
  );
}

export default App;
