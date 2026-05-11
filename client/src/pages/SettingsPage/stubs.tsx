import { SettingsSection } from './SettingsSection';

const Stub = ({ title }: { title: string }) => (
  <SettingsSection title={title}>
    <p>Coming soon.</p>
  </SettingsSection>
);

export const ProfileSettings = () => <Stub title="Profile" />;
export const AuthorizationSettings = () => <Stub title="Authorization" />;
export const StatsSettings = () => <Stub title="Stats" />;
export const MyFlightsSettings = () => <Stub title="My flights" />;
