import { Link, useParams } from 'react-router';

export function TrackPage() {
  const { id } = useParams<{ id: string }>();

  return (
    <div>
      <h1>Track: {id}</h1>
      <Link to="/">Back</Link>
    </div>
  );
}
