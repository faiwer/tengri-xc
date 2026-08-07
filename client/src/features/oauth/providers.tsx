import {
  FacebookFilled,
  GithubOutlined,
  GoogleOutlined,
  WindowsOutlined,
  XOutlined,
} from '@ant-design/icons';
import type { ComponentType } from 'react';
import type { OAuthProviderId } from '../../api/admin/oauthProviders.io';

export interface OAuthProviderMeta {
  id: OAuthProviderId;
  label: string;
  /** Brand glyph as a component so call sites can style/size it per context. */
  Icon: ComponentType<{ className?: string }>;
}

/**
 * Canonical provider catalog, in display order — shared by every OAuth surface
 * (admin credential editor, the login picker, the user's "linked accounts"
 * settings). Label + icon are presentation, so this lives in the feature
 * rather than the wire schema (`oauthProviders.io.ts`).
 */
export const OAUTH_PROVIDERS: OAuthProviderMeta[] = [
  { id: 'google', label: 'Google', Icon: GoogleOutlined },
  { id: 'facebook', label: 'Facebook', Icon: FacebookFilled },
  { id: 'x', label: 'X (Twitter)', Icon: XOutlined },
  { id: 'microsoft', label: 'Microsoft', Icon: WindowsOutlined },
  { id: 'github', label: 'GitHub', Icon: GithubOutlined },
];
