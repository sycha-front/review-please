import "./App.css";
import { ReviewActionsProvider } from "./context/ReviewActionsContext";
import Header from "./features/header/header";
import Main from "./features/main";
import { SettingsView } from "./features/settings/SettingsView";
import { pageStyle } from "./features/settings/styles";
import { useReviewDump } from "./hooks/useReviewDump";

function App() {
  const reviewState = useReviewDump();

  const { snapshot } = reviewState;

  return (
    <main style={pageStyle}>
      <Header integrations={snapshot?.integrations ?? null} />
      {snapshot && (
        <ReviewActionsProvider {...reviewState}>
          <Main data={snapshot} />
        </ReviewActionsProvider>
      )}
      <SettingsView />
    </main>
  );
}

export default App;
