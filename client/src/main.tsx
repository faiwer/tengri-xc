import { createRoot } from 'react-dom/client';
import { App } from './App';
import './index.scss';

createRoot(
  document.getElementById('root')!,
  import.meta.env.DEV
    ? // Vite eliminates this branch (and the react/debug import) in prod.
      {
        testMode: true,
        transformSource: (source) => ({
          ...source,
          fileName: source.fileName.replace(/^.+\/src/, location.origin),
        }),
      }
    : undefined,
).render(<App />);
