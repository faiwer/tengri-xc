import { GithubFilled, GoogleCircleFilled, TwitterCircleFilled, TwitterOutlined } from "@ant-design/icons";
import styles from './OAuthRow.module.scss';

export function OAuthRow() {
  return <div className={styles.row}>
    <OAuthIcon icon={<TwitterOutlined />} />
    <OAuthIcon icon={<TwitterCircleFilled />} />
    <OAuthIcon icon={<GoogleCircleFilled />} />
    <OAuthIcon icon={<GithubFilled />} />
  </div>
}

function OAuthIcon({ icon }: { icon: React.ReactNode }) {
  return <button className={styles.oauthButton}>{icon}</button>
}
