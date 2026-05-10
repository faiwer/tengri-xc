const Stub = ({ title }: { title: string }) => (
  <section>
    <h2>{title}</h2>
    <p>Coming soon.</p>
  </section>
);

export const AuthorizationSettings = () => <Stub title="Authorization" />;
export const StatsSettings = () => <Stub title="Stats" />;
export const MyFlightsSettings = () => <Stub title="My flights" />;
export const SystemSettings = () => <Stub title="System settings" />;
