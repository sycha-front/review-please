import { useEffect, useState } from "react";
import { Setting } from "../../assets/icons";
import { H1 } from "../../common/typo";
import { useAppUpdate } from "../../hooks/useAppUpdate";
import { useReleaseStatus } from "../../hooks/useReleaseStatus";
import { ReviewDump } from "../../hooks/useReviewDump";
import StatusBadge from "./components/statusBadge";
import VersionBanner from "./components/versionBanner";
import s from "./header.module.css";

type HeaderProps = {
  data: ReviewDump | null;
};

export default function Header({ data }: HeaderProps) {
  const releaseState = useReleaseStatus();
  const appUpdateState = useAppUpdate();

  function goToSetting() {
    document.getElementById("settings")?.scrollIntoView();
  }
  console.log(data);

  const [title, setTitle] = useState("LGTM👍");

  useEffect(() => {
    if (data) {
      if (data.tray_state.pending_count === 0) {
        if (data.tray_state.update_count === 0) {
          setTitle("자유🪽");
        } else {
          setTitle("LGTM👍");
        }
      } else {
      }
    } else {
      setTitle("리뷰 부탁🙏");
    }
  }, [data]);

  return (
    <>
      <header className={s.header}>
        <H1>
          {title}
          <button
            type="button"
            className={s.settingButton}
            onClick={goToSetting}
          >
            <Setting />
          </button>
        </H1>
        {data && data.integrations && (
          <div className={s.statusRow}>
            <StatusBadge
              label="GitHub"
              integration={data.integrations.github}
            />
            <StatusBadge label="Slack" integration={data.integrations.slack} />
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
