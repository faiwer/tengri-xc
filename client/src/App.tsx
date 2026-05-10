import { App as AntdApp, ConfigProvider, type ThemeConfig } from 'antd';
import { BrowserRouter, Route, Routes } from 'react-router';
import { IdentityProvider } from './core/identity';
import { PreferencesProvider } from './core/preferences';
import { LoginPage } from './pages/LoginPage';
import {
  AuthorizationSettings,
  MyFlightsSettings,
  PreferencesSettings,
  ProfileSettings,
  SettingsLayout,
  StatsSettings,
  SystemSettings,
  UserDetailSettings,
  UsersSettings,
} from './pages/SettingsPage';
import { TracksPage } from './pages/TracksPage';
import { TrackPage } from './pages/TrackPage';
import styles from './App.module.scss';

const theme: ThemeConfig = {
  token: {
    colorPrimary: '#3b82f6',
    colorBorder: '#e3e3e7',
    borderRadius: 6,
  },
};

export function App() {
  return (
    <ConfigProvider theme={theme}>
      <AntdApp className={styles.container}>
        <IdentityProvider>
          <PreferencesProvider>
            <BrowserRouter>
              <Routes>
                <Route path="/" element={<TracksPage />} />
                <Route path="/flights" element={<TracksPage />} />
                <Route path="/login" element={<LoginPage />} />
                <Route path="/flight/:id" element={<TrackPage />} />
                <Route path="/settings" element={<SettingsLayout />}>
                  <Route path="profile" element={<ProfileSettings />} />
                  <Route path="preferences" element={<PreferencesSettings />} />
                  <Route
                    path="authorization"
                    element={<AuthorizationSettings />}
                  />
                  <Route path="stats" element={<StatsSettings />} />
                  <Route path="my-flights" element={<MyFlightsSettings />} />
                  <Route path="system" element={<SystemSettings />} />
                  <Route path="users" element={<UsersSettings />} />
                  <Route path="users/:id" element={<UserDetailSettings />} />
                </Route>
              </Routes>
            </BrowserRouter>
          </PreferencesProvider>
        </IdentityProvider>
      </AntdApp>
    </ConfigProvider>
  );
}
