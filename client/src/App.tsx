import { BrowserRouter, Route, Routes } from 'react-router';
import { HomePage } from './pages/HomePage';
import { TrackPage } from './pages/TrackPage';
import styles from './App.module.scss';

export function App() {
  return (
    <BrowserRouter>
      <div className={styles.container}>
        <Routes>
          <Route path="/" element={<HomePage />} />
          <Route path="/track/:id" element={<TrackPage />} />
        </Routes>
      </div>
    </BrowserRouter>
  );
}
