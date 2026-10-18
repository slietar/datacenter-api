export function formatRelativeInstant(timestamp: number) {
  const now = Temporal.Now.zonedDateTimeISO();
  const then = Temporal.Instant.fromEpochMilliseconds(timestamp).toZonedDateTimeISO(now.timeZoneId);

  const diff = then.until(now, {
    largestUnit: 'years',
    smallestUnit: 'seconds',
  });

  const rtf = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });

  for (const unit of ['years', 'months', 'weeks', 'days', 'hours', 'minutes'] as const) {
    const value = diff[unit];
    if (value !== 0) return rtf.format(-value, unit);
  }
  return rtf.format(-diff.seconds, 'seconds');
}
