import { App as AntdApp, ConfigProvider, type ThemeConfig } from 'antd';
import { BrowserRouter, Route, Routes } from 'react-router';
import { IdentityProvider } from './core/identity';
import { LoginPage } from './pages/LoginPage';
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
          <BrowserRouter>
            <Routes>
              <Route path="/" element={<TracksPage />} />
              <Route path="/flights" element={<TracksPage />} />
              <Route path="/login" element={<LoginPage />} />
              <Route path="/track/:id" element={<TrackPage />} />
            </Routes>
          </BrowserRouter>
        </IdentityProvider>
      </AntdApp>
    </ConfigProvider>
  );
}
