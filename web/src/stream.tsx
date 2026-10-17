import { useEffect, useState } from 'react';

import { ModernEventSource } from './sse.js';

export type Stream<T> = () => T;

export async function createStream<T>(url: URL | string) {
  let eventSource = new ModernEventSource<T>(url.toString());

  await eventSource.ready;

  let initialState = await eventSource.event();

  return (() => {
    let [state, setState] = useState<T | null>(initialState);

    useEffect(() => {
      (async () => {
        for await (let event of eventSource.events()) {
          setState(event);
        }
      })();

      return () => {
        eventSource.close();
      };
    }, []);

    return state;
  }) as Stream<T>;
}
