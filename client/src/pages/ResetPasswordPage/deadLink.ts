/**
 * Why there's nothing left to submit. `offerFresh` is false when a new link
 * wouldn't help either.
 */
export interface DeadLink {
  message: string;
  offerFresh: boolean;
}

export const MISSING_TOKEN: DeadLink = {
  message: "That link is missing its token, so there's nothing to unlock here.",
  offerFresh: true,
};

/** Keyed by the `error` code the confirm endpoint returns on a 403. */
export const DEAD_LINKS: Record<string, DeadLink> = {
  reset_link_expired: {
    message: 'That link has expired. Links are valid for 24 hours.',
    offerFresh: true,
  },
  reset_link_used: {
    message:
      "That link isn't valid any more. It was already used, replaced by a newer one, or the password has changed since.",
    offerFresh: true,
  },
  account_disabled: {
    message: 'This account is disabled, so its password cannot be changed.',
    offerFresh: false,
  },
};
