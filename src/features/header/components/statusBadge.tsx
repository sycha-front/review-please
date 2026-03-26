import { Github, Slack } from "../../../assets/icons";
import Tooltip from "../../../common/tooltip";
import { P3 } from "../../../common/typo";
import { IntegrationsSummary } from "../../../hooks/useReviewDump";
import cn from "../../../utils/cn";
import s from "./statusBadge.module.css";

export default function StatusBadge({
  label,
  integration,
}: {
  label: string;
  integration: IntegrationsSummary["github"];
}) {
  return (
    <Tooltip
      message={integration.status === "error" ? integration.last_error : null}
    >
      <section
        className={cn(
          s.card,
          integration.status === "connected"
            ? s.connected
            : integration.status === "error"
              ? s.error
              : s.waiting,
        )}
      >
        {label === "GitHub" ? <Github /> : <Slack />}
        <P3>{integration.last_success_label?.split(" ")[1]}</P3>
      </section>
    </Tooltip>
  );
}
