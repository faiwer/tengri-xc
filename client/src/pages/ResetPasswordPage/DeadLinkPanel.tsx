import { Button, Typography } from 'antd';
import { useNavigate } from 'react-router';
import { routes } from '../../core/routes';
import { ResetPasswordForm } from '../../features/login/ResetPasswordForm';
import type { DeadLink } from './deadLink';

/** Says why the link is spent and offers the one thing that helps: a new one. */
export function DeadLinkPanel({ dead }: { dead: DeadLink }) {
  const navigate = useNavigate();
  const goHome = () => navigate(routes.home());

  return (
    <>
      <Typography.Paragraph>{dead.message}</Typography.Paragraph>
      {dead.offerFresh ? (
        <ResetPasswordForm onClose={goHome} />
      ) : (
        <Button type="primary" onClick={goHome} block>
          Back to flights
        </Button>
      )}
    </>
  );
}
