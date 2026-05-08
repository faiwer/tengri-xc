import { App as AntdApp, ConfigProvider, type ThemeConfig } from 'antd';
import { BrowserRouter, Route, Routes } from 'react-router';
import { TracksPage } from './pages/TracksPage';
import { TrackPage } from './pages/TrackPage';
import styles from './App.module.scss';

// Match antd's tokens to the existing palette so handrolled surfaces
// (TrackMetaPanel, AltitudeChart canvas) and antd surfaces (Segmented,
// future tables/forms) read as one design system.
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
          <Routes>
            <Route path="/" element={<TracksPage />} />
            <Route path="/track/:id" element={<TrackPage />} />
          </Routes>
        </BrowserRouter>
      </AntdApp>
    </ConfigProvider>
  );
}
