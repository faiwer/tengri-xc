export const altitudeLabel = (
  prefs: Pick<ResolvedPreferences, 'units'>,
): string => (prefs.units === 'imperial' ? 'ft' : 'm');
