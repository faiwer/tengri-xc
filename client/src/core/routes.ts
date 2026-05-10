export const routes = {
  home: () => '/',
  flights: () => '/flights',
  login: () => '/login',
  flight: (id: string) => `/flight/${id}`,
  settings: {
    index: () => '/settings',
    profile: () => '/settings/profile',
    preferences: () => '/settings/preferences',
    authorization: () => '/settings/authorization',
    stats: () => '/settings/stats',
    myFlights: () => '/settings/my-flights',
    system: () => '/settings/system',
    users: () => '/settings/users',
    user: (id: number) => `/settings/users/${id}`,
  },
};
