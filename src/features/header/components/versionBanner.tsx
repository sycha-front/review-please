import { openUrl } from "@tauri-apps/plugin-opener";

import Button from "../../../common/button";
import Tooltip from "../../../common/tooltip";
import { P3 } from "../../../common/typo";
import type { ReleaseStatus } from "../../../hooks/useReleaseStatus";
import s from "./versionBanner.module.css";

type VersionBannerProps = {
  releaseStatus: ReleaseStatus | null;
  isUpdating: boolean;
  updateError: string | null;
  onUpdate: () => Promise<void>;
};

export default function VersionBanner({
  releaseStatus,
  isUpdating,
  updateError,
  onUpdate,
}: VersionBannerProps) {
  if (!releaseStatus) {
    return null;
  }

  if (releaseStatus.error) {
    return (
      <Tooltip message={releaseStatus.error}>
        <div className={s.versionBanner}>
          <P3>버전 확인에 실패했어요.</P3>
        </div>
      </Tooltip>
    );
  }

  if (!releaseStatus.isUpdateAvailable || !releaseStatus.latestVersion) {
    return null;
  }

  return (
    <div className={s.versionBanner}>
      <div>
        {/* <P3 className={s.versionTitle}>
          현재 버전: v{releaseStatus.currentVersion}
        </P3> */}
        {updateError && <p className={s.versionError}>{updateError}</p>}
      </div>
      <div className={s.versionActions}>
        <Button
          className={s.versionButton}
          disabled={isUpdating}
          onClick={() => void onUpdate()}
          type="button"
        >
          {isUpdating
            ? "업데이트 준비 중..."
            : `v${releaseStatus.latestVersion} 업데이트`}
        </Button>
        {releaseStatus.latestReleaseUrl && (
          <Button
            className={s.versionButton}
            onClick={() => void openUrl(releaseStatus.latestReleaseUrl!)}
            type="button"
          >
            릴리즈 열기
          </Button>
        )}
      </div>
    </div>
  );
}
