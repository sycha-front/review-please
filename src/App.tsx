import "./App.css";
import s from "./App.module.css";
import { ReviewActionsProvider } from "./context/ReviewActionsContext";
import Header from "./features/header/header";
import Main from "./features/main";
import { SettingsView } from "./features/settings/SettingsView";
import { useReviewDump } from "./hooks/useReviewDump";

function App() {
  const reviewState = useReviewDump();

  const { snapshot } = reviewState;

  return (
    <main className={s.page}>
      <Header data={snapshot} />
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
