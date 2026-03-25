import { H1 } from "../../common/typo";
import type { IntegrationsSummary } from "../../hooks/useReviewDump";
import StatusBadge from "./components/statusBadge";
import s from "./header.module.css";

type HeaderProps = {
  integrations: IntegrationsSummary | null;
};

export default function Header({ integrations }: HeaderProps) {
  return (
    <header className={s.header}>
      <H1>Review-please</H1>
      {integrations && (
        <div className={s.statusRow}>
          <StatusBadge label="GitHub" integration={integrations.github} />
          <StatusBadge label="Slack" integration={integrations.slack} />
        </div>
      )}
    </header>
  );
}
