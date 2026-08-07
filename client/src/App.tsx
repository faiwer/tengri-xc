import { App as AntdApp, ConfigProvider, type ThemeConfig } from 'antd';
import type { ComponentType, ReactNode } from 'react';
import { BrowserRouter, Route, Routes } from 'react-router';
import { IdentityProvider } from './core/identity';
import { PreferencesProvider } from './core/preferences';
import { SiteProvider } from './core/site';
import { LoginProvider } from './features/login';
import { UploadFlightProvider } from './features/uploadFlight';
import { PrivacyPage, TermsPage } from './pages/DocumentPage';
import { LoginPage } from './pages/LoginPage';
import {
  AppOAuthSettings,
  AuthorizationSettings,
  GlidersSettings,
  MyFlightsSettings,
  MyGlidersSettings,
  PreferencesSettings,
  ProfileSettings,
  SettingsLayout,
  SitesSettings,
  StatsSettings,
  SystemSettings,
  UsersSettings,
} from './pages/SettingsPage';
import { TracksPage } from './pages/TracksPage';
import { TrackPage } from './pages/TrackPage';
import { ComparePage } from './pages/ComparePage';
import styles from './App.module.scss';
import { OAuthSettings } from './pages/SettingsPage/OAuthSettings';

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
        <BrowserRouter>
          <Providers>
            <Routes>
              <Route path="/" element={<TracksPage />} />
              <Route path="/flights" element={<TracksPage />} />
              <Route path="/login" element={<LoginPage />} />
              <Route path="/flight/:id" element={<TrackPage />} />
              <Route path="/compare/:ids" element={<ComparePage />} />
              <Route path="/terms" element={<TermsPage />} />
              <Route path="/privacy" element={<PrivacyPage />} />
              <Route path="/settings" element={<SettingsLayout />}>
                <Route path="profile" element={<ProfileSettings />} />
                <Route path="preferences" element={<PreferencesSettings />} />
                <Route
                  path="authorization"
                  element={
                    <>
                      <AuthorizationSettings />
                      <OAuthSettings />
                    </>
                  }
                />
                <Route path="stats" element={<StatsSettings />} />
                <Route path="my-flights" element={<MyFlightsSettings />} />
                <Route path="my-gliders" element={<MyGlidersSettings />} />
                <Route path="system" element={<SystemSettings />} />
                <Route path="oauth-providers" element={<AppOAuthSettings />} />
                <Route path="users" element={<UsersSettings />} />
                <Route path="gliders" element={<GlidersSettings />} />
                <Route path="sites" element={<SitesSettings />} />
              </Route>
            </Routes>
          </Providers>
        </BrowserRouter>
      </AntdApp>
    </ConfigProvider>
  );
}

type ProviderComponent = ComponentType<{ children: ReactNode }>;

const providers: ProviderComponent[] = [
  SiteProvider,
  IdentityProvider,
  PreferencesProvider,
  LoginProvider,
  UploadFlightProvider,
];

function Providers({ children }: { children: ReactNode }) {
  return providers.reduceRight(
    (content, Provider) => <Provider>{content}</Provider>,
    children,
  );
}
