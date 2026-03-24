import "./App.css";
import Main from "./features/main";
import { ReviewActionsProvider } from "./features/main/ReviewActionsContext";
import Header from "./features/main/components/header";
import { SettingsView } from "./features/settings/SettingsView";
import { pageStyle } from "./features/settings/styles";
import { useReviewDump } from "./hooks/useReviewDump";
import { useSettings } from "./hooks/useSettings";

function App() {
  const reviewState = useReviewDump();
  const settingsState = useSettings();

  const { snapshot } = reviewState;

  console.log(snapshot);

  return (
    <main style={pageStyle}>
      <Header />
      {snapshot && (
        <ReviewActionsProvider
          updateDeadline={reviewState.updateDeadline}
          updateStatus={reviewState.updateStatus}
        >
          <Main data={snapshot} />
        </ReviewActionsProvider>
      )}
      <SettingsView settingsState={settingsState} />
    </main>
  );
}

export default App;
