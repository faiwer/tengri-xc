export const routes = {
  home: () => '/',
  flights: () => '/flights',
  login: () => '/login',
  track: (id: string) => `/track/${id}`,
  settings: {
    index: () => '/settings',
    profile: (id: number) => `/settings/profile/${id}`,
    authorization: () => '/settings/authorization',
    stats: () => '/settings/stats',
    myFlights: () => '/settings/my-flights',
    system: () => '/settings/system',
    users: () => '/settings/users',
  },
};
