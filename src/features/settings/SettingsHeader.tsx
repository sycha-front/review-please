import {
  headerStyle,
  reviewStatusStyle,
  subtitleStyle,
  titleStyle,
} from "./styles";

type SettingsHeaderProps = {
  reviewSummary: string;
};

export function SettingsHeader({ reviewSummary }: SettingsHeaderProps) {
  return (
    <div style={headerStyle}>
      <div>
        <div style={titleStyle}>Main</div>
        <div style={subtitleStyle}>
          Settings are saved to config or keychain and reflected in the app.
        </div>
      </div>
      <div style={reviewStatusStyle}>{reviewSummary}</div>
    </div>
  );
}
