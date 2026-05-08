const dateFormatter = new Intl.DateTimeFormat(undefined, {
  day: '2-digit',
  month: '2-digit',
  year: 'numeric',
});

const timeFormatter = new Intl.DateTimeFormat(undefined, {
  hour: '2-digit',
  minute: '2-digit',
});

export const formatShortDate = (epochSeconds: number): string =>
  dateFormatter.format(new Date(epochSeconds * 1000));

export const formatShortTime = (epochSeconds: number): string =>
  timeFormatter.format(new Date(epochSeconds * 1000));

export const formatDuration = (totalSeconds: number): string => {
  const totalMinutes = Math.floor(totalSeconds / 60);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}`;
};
