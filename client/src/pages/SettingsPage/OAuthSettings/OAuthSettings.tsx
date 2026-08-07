import { OAuthRow } from '../../../features/oauth/OAuthRow';
import { SettingsSection } from '../SettingsSection';

export function OAuthSettings() {
  return (
    <SettingsSection
      title="Authorization via social networks"
      subtitle="Link your social network accounts to sign in in one click"
    >
      <OAuthRow />
    </SettingsSection>
  );
}
