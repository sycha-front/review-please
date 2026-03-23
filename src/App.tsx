import { useState } from "react";
import { SettingsView } from "./features/settings/SettingsView";
import { useReviewDump } from "./hooks/useReviewDump";
import { useSettings } from "./hooks/useSettings";

function App() {
  const reviewState = useReviewDump();
  const settingsState = useSettings();
  const [tab, setTab] = useState();

  return (
    <>
      <SettingsView reviewState={reviewState} settingsState={settingsState} />
    </>
  );
}

export default App;
