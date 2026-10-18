import ErrorIcon from '@mui/icons-material/Error';

import type { ClientResult } from './bindings/ClientResult.js';


export async function gravatarUrl(email: string) {
  let digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(email.toLowerCase()),
  );

  let hash = Array.from(new Uint8Array(digest))
    .map(b => b.toString(16).padStart(2, '0'))
    .join('');

  return `https://www.gravatar.com/avatar/${hash}?d=mp`;
}

export function mapResult<T, S>(result: ClientResult<T>, mapper: (value: T) => S) {
  if (result.ok) {
    return mapper(result.value);
  }

  return (
    <div className="InlineError">
      <span className="icon">
        <ErrorIcon />
      </span>
      Error loading data
    </div>
  )
}
