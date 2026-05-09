export const routes = {
  home: () => '/',
  flights: () => '/flights',
  login: () => '/login',
  track: (id: string) => `/track/${id}`,
};
