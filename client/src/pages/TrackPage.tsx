import { Link, useParams } from 'react-router';

export function TrackPage() {
  const { id } = useParams<{ id: string }>();

  return (
    <div>
      <h1>Track: {id}</h1>
      <p>
        Server: <code>{import.meta.env.VITE_SERVER_URL}</code>
      </p>
      <Link to="/">Back</Link>
    </div>
  );
}
