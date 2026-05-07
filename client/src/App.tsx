import { ConfigProvider, type ThemeConfig } from 'antd';
import { BrowserRouter, Route, Routes } from 'react-router';
import { HomePage } from './pages/HomePage';
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
      <BrowserRouter>
        <div className={styles.container}>
          <Routes>
            <Route path="/" element={<HomePage />} />
            <Route path="/track/:id" element={<TrackPage />} />
          </Routes>
        </div>
      </BrowserRouter>
    </ConfigProvider>
  );
}
