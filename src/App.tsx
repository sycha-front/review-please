import "./App.css";
import { ReviewActionsProvider } from "./context/ReviewActionsContext";
import Header from "./features/header/header";
import Main from "./features/main";
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
      <Header integrations={snapshot?.integrations ?? null} />
      {snapshot && (
        <ReviewActionsProvider
          updateDeadline={reviewState.updateDeadline}
          updateStatus={reviewState.updateStatus}
        >
          <Main
            data={snapshot}
            markUpdateRead={reviewState.markUpdateRead}
            markAllUpdateRead={reviewState.markAllUpdateRead}
          />
        </ReviewActionsProvider>
      )}
      <SettingsView settingsState={settingsState} />
    </main>
  );
}

export default App;
