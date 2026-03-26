import { Setting } from "../../assets/icons";
import { H1 } from "../../common/typo";
import { useAppUpdate } from "../../hooks/useAppUpdate";
import { useReleaseStatus } from "../../hooks/useReleaseStatus";
import type { IntegrationsSummary } from "../../hooks/useReviewDump";
import StatusBadge from "./components/statusBadge";
import VersionBanner from "./components/versionBanner";
import s from "./header.module.css";

type HeaderProps = {
  integrations: IntegrationsSummary | null;
};

export default function Header({ integrations }: HeaderProps) {
  const releaseState = useReleaseStatus();
  const appUpdateState = useAppUpdate();

  function goToSetting() {
    document.getElementById("settings")?.scrollIntoView();
  }

  return (
    <>
      <header className={s.header}>
        <H1>
          LGTM👍
          <button
            type="button"
            className={s.settingButton}
            onClick={goToSetting}
          >
            <Setting />
          </button>
        </H1>
        {integrations && (
          <div className={s.statusRow}>
            <StatusBadge label="GitHub" integration={integrations.github} />
            <StatusBadge label="Slack" integration={integrations.slack} />
          </div>
        )}
      </header>
      <VersionBanner
        releaseStatus={releaseState.releaseStatus}
        isUpdating={appUpdateState.isUpdating}
        updateError={appUpdateState.error}
        onUpdate={() => appUpdateState.runUpdate()}
      />
    </>
  );
}
