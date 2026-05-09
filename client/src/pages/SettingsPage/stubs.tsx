import { useParams } from 'react-router';

const Stub = ({ title }: { title: string }) => (
  <section>
    <h2>{title}</h2>
    <p>Coming soon.</p>
  </section>
);

export const ProfileSettings = () => {
  const { id } = useParams<{ id: string }>();
  return <Stub title={`Profile #${id ?? '?'}`} />;
};

export const AuthorizationSettings = () => <Stub title="Authorization" />;
export const StatsSettings = () => <Stub title="Stats" />;
export const MyFlightsSettings = () => <Stub title="My flights" />;
export const SystemSettings = () => <Stub title="System settings" />;
