import { Copiable } from '@opsa-dev/ui-tscl';

import { formatRelativeInstant } from '../formatting.js';
import type { HostsClientState } from '../bindings/HostsClientState.js';
import { Layout } from '../layout.js';
import { createStream, type Stream } from '../stream.js';
import type { ClientResult } from '../bindings/ClientResult.js';


export async function handleHomePage() {
  let stream = await createStream<HostsClientState>('http://localhost:8080/hosts/events');

  return {
    contents: (
      <Layout activeNavigationEntryId="servers">
        <HomePage useHostsStream={stream} />
      </Layout>
    ),
    title: 'Home',
  };
}

function mapResult<T, S>(result: ClientResult<T>, mapper: (value: T) => S) {
  if (result.ok) {
    return mapper(result.value);
  }

  return <>Error</>;
}

export function HomePage(props: {
  useHostsStream: Stream<HostsClientState>,
}) {
  let state = props.useHostsStream();

  return (
    <>
      <h1>Machines</h1>

      <table className="Table">
        <thead>
          <tr>
            <th>Name</th>
            <th>IP addresses</th>
            <th>Headscale status</th>
            <th>IPMI status</th>
          </tr>
        </thead>
        <tbody>
          {Object.values(state?.hosts ?? {})
            .toSorted((a, b) => a.name.localeCompare(b.name))
            .map((host) => (
              <tr key={host.name}>
                <td>{host.name}</td>
                <td>
                  <Copiable contents={host.address}>{host.address}</Copiable>
                </td>
                <td>
                  {host.headscale
                    ? mapResult(host.headscale, (headscaleState) => (
                      headscaleState
                        ? headscaleState.node.online
                          ? <>Online</>
                          : <>Offline, last seen {formatRelativeInstant(headscaleState.node.lastVisibilityTime * 1000)}</>
                        : <>Not found</>
                    ))
                    : <>Not configured</>}
                </td>
                <td>
                  {host.ipmi
                    ? mapResult(host.ipmi, (ipmiState) => (
                      ipmiState.powerIsOn
                        ? <>Powered on</>
                        : <>Powered off</>
                    ))
                    : <>Not configured</>}
                </td>
              </tr>
            ))}
        </tbody>
      </table>

      <pre>{JSON.stringify(state, null, 2)}</pre>
    </>
  );
}
