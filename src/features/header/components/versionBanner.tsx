import { openUrl } from "@tauri-apps/plugin-opener";

import Button from "../../../common/button";
import type { ReleaseStatus } from "../../../hooks/useReleaseStatus";
import s from "../header.module.css";

type VersionBannerProps = {
  releaseStatus: ReleaseStatus | null;
  repoPath: string;
  isUpdating: boolean;
  updateError: string | null;
  onUpdate: () => Promise<void>;
};

export default function VersionBanner({
  releaseStatus,
  repoPath,
  isUpdating,
  updateError,
  onUpdate,
}: VersionBannerProps) {
  if (!releaseStatus) {
    return null;
  }

  if (releaseStatus.error) {
    return (
      <div className={s.versionBanner}>
        <p className={s.versionTitle}>버전 확인에 실패했어요.</p>
        <p className={s.versionDesc}>{releaseStatus.error}</p>
      </div>
    );
  }

  if (!releaseStatus.isUpdateAvailable || !releaseStatus.latestVersion) {
    return null;
  }

  return (
    <div className={s.versionBanner}>
      <div>
        <p className={s.versionTitle}>
          새 버전 {releaseStatus.latestVersion} 이 있습니다.
        </p>
        <p className={s.versionDesc}>
          현재 버전 {releaseStatus.currentVersion} 에서 업데이트할 수 있어요.
        </p>
        {updateError && <p className={s.versionError}>{updateError}</p>}
      </div>
      <div className={s.versionActions}>
        {repoPath && (
          <Button
            className={s.versionButton}
            disabled={isUpdating}
            onClick={() => void onUpdate()}
            type="button"
          >
            {isUpdating ? "업데이트 준비 중..." : "원클릭 업데이트"}
          </Button>
        )}
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
